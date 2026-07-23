use {
    super::{
        ingestor::{EventIngestionError, EventIngestor, EventPage},
        metrics::*,
        query::EventQuery,
    },
    crate::sui,
    futures::TryStreamExt as _,
    std::time::{Duration, Instant},
    tokio::sync::mpsc,
};

const INDEX_PROGRESS_DELAY: Duration = Duration::from_millis(50);
#[cfg(not(test))]
pub(super) const RECONNECT_DELAY: Duration = Duration::from_secs(2);
#[cfg(test)]
pub(super) const RECONNECT_DELAY: Duration = Duration::from_millis(10);

impl<Q: EventQuery> EventIngestor<Q> {
    pub(super) async fn run(
        self,
        from_checkpoint: Option<u64>,
        send_page: mpsc::Sender<Result<EventPage<Q::Output>, EventIngestionError>>,
    ) {
        let mut resume_checkpoint = from_checkpoint;
        let mut highest_output_checkpoint = None;

        loop {
            match self
                .run_connection(
                    &mut resume_checkpoint,
                    &mut highest_output_checkpoint,
                    &send_page,
                )
                .await
            {
                Ok(()) => return,
                Err(error) => {
                    let should_reconnect = error.is_retryable();
                    if !self.send_error(error, &send_page).await
                        || !should_reconnect
                        || !self.wait(RECONNECT_DELAY, &send_page).await
                    {
                        return;
                    }
                    STREAM_RECONNECTIONS.inc();
                }
            }
        }
    }

    async fn run_connection(
        &self,
        resume_checkpoint: &mut Option<u64>,
        highest_output_checkpoint: &mut Option<u64>,
        send_page: &mpsc::Sender<Result<EventPage<Q::Output>, EventIngestionError>>,
    ) -> Result<(), EventIngestionError> {
        let mut client = sui::grpc::client(&self.rpc_url).map_err(|error| {
            EventIngestionError::Configuration(format!(
                "invalid gRPC URL '{}': {error}",
                self.rpc_url
            ))
        })?;
        let request = sui::grpc::SubscribeEventsRequest::default()
            .with_read_mask(self.read_mask.clone())
            .with_filter(self.filter.clone());

        let mut subscription_client = client.subscription_client();
        let response = tokio::select! {
            _ = self.cancellation_token.cancelled() => return Ok(()),
            _ = send_page.closed() => return Ok(()),
            response = subscription_client.subscribe_events(request) => response,
        }
        .map_err(|status| EventIngestionError::rpc("subscribing to events", status))?;
        let mut stream = response.into_inner();

        let first = tokio::select! {
            _ = self.cancellation_token.cancelled() => return Ok(()),
            _ = send_page.closed() => return Ok(()),
            response = stream.try_next() => response,
        }
        .map_err(|status| EventIngestionError::rpc("establishing the event stream", status))?
        .ok_or_else(|| {
            EventIngestionError::rpc(
                "establishing the event stream",
                tonic::Status::unavailable("event stream ended before its start cursor"),
            )
        })?;

        let live_start_cursor = Self::response_cursor(first.watermark.as_ref())?.to_vec();
        if let Some(start_checkpoint) = *resume_checkpoint {
            let replay_complete = self
                .replay_events(
                    &mut client,
                    start_checkpoint,
                    live_start_cursor,
                    resume_checkpoint,
                    highest_output_checkpoint,
                    send_page,
                )
                .await?;
            if !replay_complete {
                return Ok(());
            }
        }

        if !self
            .process_frame(
                first.event,
                first.watermark.as_ref(),
                resume_checkpoint,
                highest_output_checkpoint,
                send_page,
            )
            .await?
        {
            return Ok(());
        }

        loop {
            let response = tokio::select! {
                _ = self.cancellation_token.cancelled() => return Ok(()),
                _ = send_page.closed() => return Ok(()),
                response = stream.try_next() => response,
            }
            .map_err(|status| EventIngestionError::rpc("receiving an event stream frame", status))?
            .ok_or_else(|| {
                EventIngestionError::rpc(
                    "receiving an event stream frame",
                    tonic::Status::unavailable("event stream ended unexpectedly"),
                )
            })?;

            if !self
                .process_frame(
                    response.event,
                    response.watermark.as_ref(),
                    resume_checkpoint,
                    highest_output_checkpoint,
                    send_page,
                )
                .await?
            {
                return Ok(());
            }
        }
    }

    async fn replay_events(
        &self,
        client: &mut sui::grpc::Client,
        start_checkpoint: u64,
        live_start_cursor: Vec<u8>,
        resume_checkpoint: &mut Option<u64>,
        highest_output_checkpoint: &mut Option<u64>,
        send_page: &mpsc::Sender<Result<EventPage<Q::Output>, EventIngestionError>>,
    ) -> Result<bool, EventIngestionError> {
        let mut after_cursor: Option<Vec<u8>> = None;

        loop {
            let mut options =
                sui::grpc::QueryOptions::default().with_before(live_start_cursor.clone());
            if let Some(after) = &after_cursor {
                options.set_after(after.clone());
            }

            let request = sui::grpc::ListEventsRequest::default()
                .with_read_mask(self.read_mask.clone())
                .with_start_checkpoint(start_checkpoint)
                .with_filter(self.filter.clone())
                .with_options(options);
            REPLAY_REQUESTS.inc();
            let mut ledger_client = client.ledger_client();
            let response = tokio::select! {
                _ = self.cancellation_token.cancelled() => return Ok(false),
                _ = send_page.closed() => return Ok(false),
                response = ledger_client.list_events(request) => response,
            }
            .map_err(|status| EventIngestionError::rpc("requesting event replay", status))?;
            let mut stream = response.into_inner();

            let terminal_reason = loop {
                let response = tokio::select! {
                    _ = self.cancellation_token.cancelled() => return Ok(false),
                    _ = send_page.closed() => return Ok(false),
                    response = stream.try_next() => response,
                }
                .map_err(|status| {
                    EventIngestionError::rpc("receiving an event replay frame", status)
                })?
                .ok_or_else(|| {
                    EventIngestionError::Protocol(
                        "event replay ended without a terminal frame".to_owned(),
                    )
                })?;

                after_cursor = Some(Self::response_cursor(response.watermark.as_ref())?.to_vec());
                let end = response
                    .end
                    .as_ref()
                    .and_then(|end| end.reason)
                    .and_then(|reason| sui::grpc::QueryEndReason::try_from(reason).ok());
                if !self
                    .process_frame(
                        response.event,
                        response.watermark.as_ref(),
                        resume_checkpoint,
                        highest_output_checkpoint,
                        send_page,
                    )
                    .await?
                {
                    return Ok(false);
                }
                if let Some(reason) = end {
                    break reason;
                }
            };

            match terminal_reason {
                sui::grpc::QueryEndReason::CursorBound
                | sui::grpc::QueryEndReason::CheckpointBound => {
                    return Ok(true);
                }
                sui::grpc::QueryEndReason::ItemLimit | sui::grpc::QueryEndReason::ScanLimit => {}
                sui::grpc::QueryEndReason::LedgerTip => {
                    if !self.wait(INDEX_PROGRESS_DELAY, send_page).await {
                        return Ok(false);
                    }
                }
                reason => {
                    return Err(EventIngestionError::Protocol(format!(
                        "event replay stopped for an unsupported reason: \
                         {reason:?}"
                    )));
                }
            }
        }
    }

    async fn process_frame(
        &self,
        event: Option<sui::grpc::Event>,
        watermark: Option<&sui::grpc::Watermark>,
        resume_checkpoint: &mut Option<u64>,
        highest_output_checkpoint: &mut Option<u64>,
        send_page: &mpsc::Sender<Result<EventPage<Q::Output>, EventIngestionError>>,
    ) -> Result<bool, EventIngestionError> {
        let watermark = watermark.ok_or_else(|| {
            EventIngestionError::Protocol("event frame is missing its watermark".to_owned())
        })?;
        if watermark.cursor_opt().is_none() {
            return Err(EventIngestionError::Protocol(
                "event frame is missing its cursor".to_owned(),
            ));
        }

        if let Some(checkpoint) = watermark.checkpoint_opt() {
            Self::advance_checkpoint(resume_checkpoint, checkpoint);
            if highest_output_checkpoint.is_none_or(|current| checkpoint > current) {
                if !self
                    .send_page(
                        EventPage {
                            events: Vec::new(),
                            checkpoint,
                        },
                        send_page,
                    )
                    .await
                {
                    return Ok(false);
                }
                Self::advance_checkpoint(highest_output_checkpoint, checkpoint);
            }
        }

        let Some(event) = event else {
            return Ok(true);
        };
        FILTERED_EVENTS_RECEIVED.inc();
        let checkpoint = event.checkpoint.ok_or_else(|| {
            EventIngestionError::Protocol("event is missing its checkpoint".to_owned())
        })?;
        let transaction_digest = event.transaction_digest.clone().ok_or_else(|| {
            EventIngestionError::Protocol("event is missing its transaction digest".to_owned())
        })?;
        transaction_digest
            .parse::<sui::types::Digest>()
            .map_err(|error| {
                EventIngestionError::Protocol(format!(
                    "event has an invalid transaction digest: {error}"
                ))
            })?;
        let event_index = event.event_index.ok_or_else(|| {
            EventIngestionError::Protocol("event is missing its event index".to_owned())
        })?;

        let decode_started = Instant::now();
        let decoded = self.query.decode(event);
        EVENT_PARSE_DURATION.observe(decode_started.elapsed().as_secs_f64());
        let event = decoded.map_err(|source| EventIngestionError::Decode {
            checkpoint,
            transaction_digest,
            event_index,
            source: anyhow::Error::new(source),
        })?;
        let events = event.into_iter().collect::<Vec<_>>();
        EVENTS_PER_PAGE.observe(events.len() as f64);
        Self::advance_checkpoint(resume_checkpoint, checkpoint);
        let should_send = !events.is_empty()
            || highest_output_checkpoint.is_none_or(|current| checkpoint > current);
        if should_send {
            if !self
                .send_page(EventPage { events, checkpoint }, send_page)
                .await
            {
                return Ok(false);
            }
            Self::advance_checkpoint(highest_output_checkpoint, checkpoint);
        }

        Ok(true)
    }

    async fn send_error(
        &self,
        error: EventIngestionError,
        send_page: &mpsc::Sender<Result<EventPage<Q::Output>, EventIngestionError>>,
    ) -> bool {
        tokio::select! {
            _ = self.cancellation_token.cancelled() => false,
            _ = send_page.closed() => false,
            result = send_page.send(Err(error)) => result.is_ok(),
        }
    }

    async fn send_page(
        &self,
        page: EventPage<Q::Output>,
        send_page: &mpsc::Sender<Result<EventPage<Q::Output>, EventIngestionError>>,
    ) -> bool {
        let started = Instant::now();
        let sent = tokio::select! {
            _ = self.cancellation_token.cancelled() => false,
            _ = send_page.closed() => false,
            result = send_page.send(Ok(page)) => result.is_ok(),
        };
        SEND_PAGE_BACKPRESSURE_DURATION.observe(started.elapsed().as_secs_f64());
        EVENT_PAGE_CHANNEL_LEN.set((send_page.max_capacity() - send_page.capacity()) as f64);
        sent
    }

    async fn wait(
        &self,
        duration: Duration,
        send_page: &mpsc::Sender<Result<EventPage<Q::Output>, EventIngestionError>>,
    ) -> bool {
        tokio::select! {
            _ = self.cancellation_token.cancelled() => false,
            _ = send_page.closed() => false,
            _ = tokio::time::sleep(duration) => true,
        }
    }

    fn response_cursor(
        watermark: Option<&sui::grpc::Watermark>,
    ) -> Result<&[u8], EventIngestionError> {
        watermark
            .and_then(sui::grpc::Watermark::cursor_opt)
            .ok_or_else(|| {
                EventIngestionError::Protocol("event frame is missing its cursor".to_owned())
            })
    }

    fn advance_checkpoint(checkpoint: &mut Option<u64>, candidate: u64) {
        if checkpoint.is_none_or(|current| candidate > current) {
            *checkpoint = Some(candidate);
        }
    }
}
