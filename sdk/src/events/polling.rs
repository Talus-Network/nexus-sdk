//! Nexus event polling through the filtered Sui gRPC event APIs.
//!
//! Each [`EventPoller`] owns one live subscription for one Nexus deployment.
//! Historical gaps are replayed with [`sui::grpc::ListEventsRequest`] before
//! the live stream is consumed. Both paths use the same Nexus wrapper type
//! filter.
//!
//! See also: <https://github.com/Talus-Network/nexus/issues/724>

use {
    crate::{
        events::{FromSuiGrpcEvent, NexusEvent},
        move_bindings::primitives::{
            data::NexusData as MoveNexusData,
            distributed_event as distributed_event_move,
            event as event_move,
        },
        sui,
        types::NexusObjects,
    },
    futures::TryStreamExt,
    std::{sync::Arc, time::Duration},
    sui_rpc::{field::FieldMaskUtil, proto::sui::rpc::v2::filter::event as event_filter},
    thiserror::Error,
    tokio::sync::mpsc,
    tokio_util::sync::CancellationToken,
};

lazy_static::lazy_static! {
    static ref STREAM_RECONNECTIONS: prometheus::Counter = prometheus::register_counter!(
        "poller_stream_reconnections",
        "Number of filtered event stream reconnections"
    ).unwrap();

    static ref REPLAY_REQUESTS: prometheus::Counter = prometheus::register_counter!(
        "poller_event_replay_requests",
        "Number of filtered event replay requests"
    ).unwrap();

    static ref FILTERED_EVENTS_RECEIVED: prometheus::Counter = prometheus::register_counter!(
        "poller_filtered_events_received",
        "Number of events received after the Nexus wrapper filter"
    ).unwrap();

    static ref EVENT_PARSE_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_event_parse_duration",
        "Duration of Nexus event parsing [s]",
        vec![0.00001, 0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05]
    ).unwrap();

    static ref EVENTS_PER_PAGE: prometheus::Histogram = prometheus::register_histogram!(
        "poller_events_per_page",
        "Number of Nexus events per event page",
        vec![0.0, 1.0]
    ).unwrap();

    static ref EVENT_PAGE_CHANNEL_LEN: prometheus::Gauge = prometheus::register_gauge!(
        "poller_event_page_channel_len",
        "Number of event pages waiting for the consumer"
    ).unwrap();

    static ref SEND_PAGE_BACKPRESSURE_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_send_page_backpressure_duration",
        "Duration blocked while sending an event page [s]",
        vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]
    ).unwrap();
}

#[cfg(not(test))]
const RECONNECT_DELAY: Duration = Duration::from_secs(2);
#[cfg(test)]
const RECONNECT_DELAY: Duration = Duration::from_millis(10);
const INDEX_PROGRESS_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug, Error)]
pub enum PollerError {
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("gRPC error: {0}")]
    Rpc(anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct EventPage {
    pub events: Vec<NexusEvent>,
    pub checkpoint: u64,
}

#[derive(Clone)]
pub struct EventPoller {
    rpc_url: String,
    nexus_objects: Arc<NexusObjects>,
    channel_capacity: usize,
    cancellation_token: CancellationToken,
}

impl EventPoller {
    pub fn new(rpc_url: &str, nexus_objects: Arc<NexusObjects>) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            nexus_objects,
            channel_capacity: 100,
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.cancellation_token = cancellation_token;
        self
    }

    /// Start polling from an inclusive checkpoint.
    ///
    /// Passing [`None`] starts at the current live stream position. Passing a
    /// checkpoint replays matching events from that checkpoint before live
    /// delivery begins.
    pub fn start_polling(
        self,
        from_checkpoint: Option<u64>,
    ) -> Result<mpsc::Receiver<Result<EventPage, PollerError>>, PollerError> {
        sui::grpc::client(&self.rpc_url).map_err(|error| {
            PollerError::Configuration(format!("Invalid gRPC URL '{}': {error}", self.rpc_url))
        })?;

        let (send_page, next_page) = mpsc::channel(self.channel_capacity);
        tokio::spawn(Arc::new(self).run(from_checkpoint, send_page));

        Ok(next_page)
    }

    async fn run(
        self: Arc<Self>,
        mut resume_checkpoint: Option<u64>,
        send_page: mpsc::Sender<Result<EventPage, PollerError>>,
    ) {
        let mut reconnecting = false;
        let mut highest_output_checkpoint = None;

        loop {
            if self.should_stop(&send_page) {
                return;
            }

            if reconnecting {
                STREAM_RECONNECTIONS.inc();
                if !self.wait(RECONNECT_DELAY, &send_page).await {
                    return;
                }
            }
            reconnecting = true;

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
                    if send_page.send(Err(error)).await.is_err() {
                        return;
                    }
                }
            }
        }
    }

    async fn run_connection(
        &self,
        resume_checkpoint: &mut Option<u64>,
        highest_output_checkpoint: &mut Option<u64>,
        send_page: &mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> Result<(), PollerError> {
        let mut client = sui::grpc::client(&self.rpc_url).map_err(|error| {
            PollerError::Configuration(format!("Invalid gRPC URL '{}': {error}", self.rpc_url))
        })?;

        let request = sui::grpc::SubscribeEventsRequest::default()
            .with_read_mask(Self::event_read_mask())
            .with_filter(self.event_filter());

        let mut subscription_client = client.subscription_client();
        let response = tokio::select! {
            _ = self.cancellation_token.cancelled() => return Ok(()),
            _ = send_page.closed() => return Ok(()),
            response = subscription_client.subscribe_events(request) => response,
        }
        .map_err(|error| {
            PollerError::Rpc(anyhow::anyhow!(
                "Failed to subscribe to Nexus events: {error}"
            ))
        })?;
        let mut live_stream = response.into_inner();

        let first = tokio::select! {
            _ = self.cancellation_token.cancelled() => return Ok(()),
            _ = send_page.closed() => return Ok(()),
            response = live_stream.try_next() => response,
        }
        .map_err(|error| {
            PollerError::Rpc(anyhow::anyhow!(
                "Failed to establish the Nexus event stream: {error}"
            ))
        })?
        .ok_or_else(|| {
            PollerError::Rpc(anyhow::anyhow!(
                "Nexus event stream ended before its start cursor"
            ))
        })?;

        let live_start_cursor = Self::response_cursor(first.watermark_opt())?.to_vec();

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
                first.event_opt(),
                first.watermark_opt(),
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
                response = live_stream.try_next() => response,
            }
            .map_err(|error| {
                PollerError::Rpc(anyhow::anyhow!(
                    "Failed to receive a Nexus event stream frame: {error}"
                ))
            })?
            .ok_or_else(|| {
                PollerError::Rpc(anyhow::anyhow!("Nexus event stream ended unexpectedly"))
            })?;

            if !self
                .process_frame(
                    response.event_opt(),
                    response.watermark_opt(),
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
        send_page: &mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> Result<bool, PollerError> {
        let mut after_cursor: Option<Vec<u8>> = None;

        loop {
            let mut options =
                sui::grpc::QueryOptions::default().with_before(live_start_cursor.clone());
            if let Some(after) = &after_cursor {
                options.set_after(after.clone());
            }

            let request = sui::grpc::ListEventsRequest::default()
                .with_read_mask(Self::event_read_mask())
                .with_start_checkpoint(start_checkpoint)
                .with_filter(self.event_filter())
                .with_options(options);

            REPLAY_REQUESTS.inc();
            let mut ledger_client = client.ledger_client();
            let response = tokio::select! {
                _ = self.cancellation_token.cancelled() => return Ok(false),
                _ = send_page.closed() => return Ok(false),
                response = ledger_client.list_events(request) => response,
            }
            .map_err(|error| {
                PollerError::Rpc(anyhow::anyhow!("Failed to replay Nexus events: {error}"))
            })?;
            let mut stream = response.into_inner();
            let terminal_reason = loop {
                let response = tokio::select! {
                    _ = self.cancellation_token.cancelled() => return Ok(false),
                    _ = send_page.closed() => return Ok(false),
                    response = stream.try_next() => response,
                }
                .map_err(|error| {
                    PollerError::Rpc(anyhow::anyhow!(
                        "Failed while replaying Nexus events: {error}"
                    ))
                })?
                .ok_or_else(|| {
                    PollerError::Rpc(anyhow::anyhow!(
                        "Nexus event replay ended without a terminal frame"
                    ))
                })?;

                after_cursor = Some(Self::response_cursor(response.watermark_opt())?.to_vec());
                if !self
                    .process_frame(
                        response.event_opt(),
                        response.watermark_opt(),
                        resume_checkpoint,
                        highest_output_checkpoint,
                        send_page,
                    )
                    .await?
                {
                    return Ok(false);
                }

                if let Some(end) = response.end_opt() {
                    let reason = end
                        .reason
                        .and_then(|reason| sui::grpc::QueryEndReason::try_from(reason).ok())
                        .ok_or_else(|| {
                            PollerError::Rpc(anyhow::anyhow!(
                                "Nexus event replay returned no terminal reason"
                            ))
                        })?;
                    break reason;
                }
            };

            match terminal_reason {
                sui::grpc::QueryEndReason::CursorBound
                | sui::grpc::QueryEndReason::CheckpointBound => return Ok(true),
                sui::grpc::QueryEndReason::ItemLimit | sui::grpc::QueryEndReason::ScanLimit => {}
                sui::grpc::QueryEndReason::LedgerTip => {
                    if !self.wait(INDEX_PROGRESS_DELAY, send_page).await {
                        return Ok(false);
                    }
                }
                reason => {
                    return Err(PollerError::Rpc(anyhow::anyhow!(
                        "Nexus event replay stopped for an unsupported reason: {reason:?}"
                    )));
                }
            }
        }
    }

    async fn process_frame(
        &self,
        event: Option<&sui::grpc::Event>,
        watermark: Option<&sui::grpc::Watermark>,
        resume_checkpoint: &mut Option<u64>,
        highest_output_checkpoint: &mut Option<u64>,
        send_page: &mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> Result<bool, PollerError> {
        let watermark = watermark.ok_or_else(|| {
            PollerError::Rpc(anyhow::anyhow!(
                "Nexus event frame is missing its watermark"
            ))
        })?;
        Self::response_cursor(Some(watermark))?;

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
            PollerError::Rpc(anyhow::anyhow!("Filtered Nexus event has no checkpoint"))
        })?;
        let digest = event
            .transaction_digest
            .as_deref()
            .ok_or_else(|| {
                PollerError::Rpc(anyhow::anyhow!(
                    "Filtered Nexus event has no transaction digest"
                ))
            })?
            .parse()
            .map_err(|error| {
                PollerError::Rpc(anyhow::anyhow!(
                    "Filtered Nexus event has an invalid transaction digest: {error}"
                ))
            })?;
        let event_index = event.event_index.ok_or_else(|| {
            PollerError::Rpc(anyhow::anyhow!("Filtered Nexus event has no event index"))
        })?;
        let sui_event = sui::types::Event::try_from(event).map_err(|error| {
            PollerError::Rpc(anyhow::anyhow!(
                "Filtered Nexus event is incomplete: {error}"
            ))
        })?;

        let parse_started = std::time::Instant::now();
        let events = match NexusEvent::from_sui_grpc_event(
            event_index.into(),
            digest,
            &sui_event,
            &self.nexus_objects,
        ) {
            Ok(event) => vec![event],
            Err(error) => {
                tracing::debug!(%error, "Ignoring a primitives package event that is not a Nexus wrapper");
                Vec::new()
            }
        };
        EVENT_PARSE_DURATION.observe(parse_started.elapsed().as_secs_f64());
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

    async fn send_page(
        &self,
        page: EventPage,
        send_page: &mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> bool {
        let started = std::time::Instant::now();
        let sent = tokio::select! {
            _ = self.cancellation_token.cancelled() => return false,
            _ = send_page.closed() => return false,
            result = send_page.send(Ok(page)) => result.is_ok(),
        };
        SEND_PAGE_BACKPRESSURE_DURATION.observe(started.elapsed().as_secs_f64());
        EVENT_PAGE_CHANNEL_LEN.set((send_page.max_capacity() - send_page.capacity()) as f64);
        sent
    }

    async fn wait(
        &self,
        duration: Duration,
        send_page: &mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> bool {
        tokio::select! {
            _ = self.cancellation_token.cancelled() => false,
            _ = send_page.closed() => false,
            _ = tokio::time::sleep(duration) => true,
        }
    }

    fn should_stop(&self, send_page: &mpsc::Sender<Result<EventPage, PollerError>>) -> bool {
        self.cancellation_token.is_cancelled() || send_page.is_closed()
    }

    fn event_filter(&self) -> sui::grpc::EventFilter {
        let wrapper = crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(
            &self.nexus_objects,
        );
        let distributed_wrapper = crate::move_bindings::struct_tag::<
            distributed_event_move::DistributedEventWrapper<MoveNexusData>,
        >(&self.nexus_objects);

        sui::grpc::EventFilter::any([wrapper, distributed_wrapper].map(|tag| {
            event_filter::event_type(format!(
                "{}::{}::{}",
                tag.address(),
                tag.module(),
                tag.name()
            ))
        }))
    }

    fn event_read_mask() -> sui::grpc::FieldMask {
        sui::grpc::FieldMask::from_paths([
            "package_id",
            "module",
            "sender",
            "event_type",
            "contents",
            "checkpoint",
            "transaction_digest",
            "event_index",
        ])
    }

    fn response_cursor(watermark: Option<&sui::grpc::Watermark>) -> Result<&[u8], PollerError> {
        watermark
            .and_then(sui::grpc::Watermark::cursor_opt)
            .ok_or_else(|| {
                PollerError::Rpc(anyhow::anyhow!("Nexus event frame is missing its cursor"))
            })
    }

    fn advance_checkpoint(checkpoint: &mut Option<u64>, candidate: u64) {
        if checkpoint.is_none_or(|current| candidate > current) {
            *checkpoint = Some(candidate);
        }
    }
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            events::NexusEventKind,
            move_bindings::{
                interface::graph::{OutputVariant, RuntimeVertex},
                move_std::ascii::String as MoveString,
                primitives::{data::NexusData, event as event_move},
                sui_framework::object::ID,
                workflow::execution_events::WalkAdvancedEvent,
            },
            test_utils::sui_mocks,
        },
        futures::StreamExt as _,
        serde::Serialize,
        std::sync::atomic::{AtomicUsize, Ordering},
        sui_rpc::proto::sui::rpc::v2::event_literal::Predicate,
        tokio::time::timeout,
    };

    fn id(bytes: sui::types::Address) -> ID {
        ID { bytes }
    }

    fn walk_event(objects: &NexusObjects, checkpoint: u64) -> sui::grpc::Event {
        #[derive(Serialize)]
        struct Wrapper<T> {
            event: T,
        }

        let event = WalkAdvancedEvent {
            dag: id(sui_mocks::mock_sui_address()),
            execution: id(sui_mocks::mock_sui_address()),
            walk_index: 0,
            vertex: RuntimeVertex::plain("v"),
            variant: OutputVariant {
                name: MoveString::from("ok"),
            },
            variant_ports_to_data: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
        };
        let wrapper_tag =
            crate::move_bindings::struct_tag::<event_move::EventWrapper<NexusData>>(objects);
        let event_type = format!(
            "{}::{}::{}<{}::execution_events::WalkAdvancedEvent>",
            objects.primitives_pkg_id,
            wrapper_tag.module(),
            wrapper_tag.name(),
            objects.workflow_pkg_id,
        );

        let mut grpc_event = sui::grpc::Event::default();
        grpc_event.set_package_id(objects.workflow_pkg_id);
        grpc_event.set_module("execution_events");
        grpc_event.set_sender(sui::types::Address::ZERO);
        grpc_event.set_event_type(event_type);
        grpc_event.set_contents(bcs::to_bytes(&Wrapper { event }).unwrap());
        grpc_event.set_checkpoint(checkpoint);
        grpc_event.set_transaction_digest(sui::types::Digest::ZERO);
        grpc_event.set_event_index(0);
        grpc_event
    }

    fn watermark(cursor: &[u8], checkpoint: Option<u64>) -> sui::grpc::Watermark {
        let mut watermark = sui::grpc::Watermark::default();
        watermark.set_cursor(cursor.to_vec());
        if let Some(checkpoint) = checkpoint {
            watermark.set_checkpoint(checkpoint);
        }
        watermark
    }

    fn subscription_frame(
        event: Option<sui::grpc::Event>,
        watermark: sui::grpc::Watermark,
    ) -> sui::grpc::SubscribeEventsResponse {
        let mut response = sui::grpc::SubscribeEventsResponse::default();
        if let Some(event) = event {
            response.set_event(event);
        }
        response.set_watermark(watermark);
        response
    }

    fn list_frame(
        event: Option<sui::grpc::Event>,
        watermark: sui::grpc::Watermark,
        end: Option<sui::grpc::QueryEndReason>,
    ) -> sui::grpc::ListEventsResponse {
        let mut response = sui::grpc::ListEventsResponse::default();
        if let Some(event) = event {
            response.set_event(event);
        }
        response.set_watermark(watermark);
        if let Some(reason) = end {
            let mut query_end = sui::grpc::QueryEnd::default();
            query_end.set_reason(reason);
            response.set_end(query_end);
        }
        response
    }

    fn wrapper_event_types(objects: &NexusObjects) -> [String; 2] {
        [
            crate::move_bindings::struct_tag::<event_move::EventWrapper<NexusData>>(objects),
            crate::move_bindings::struct_tag::<
                crate::move_bindings::primitives::distributed_event::DistributedEventWrapper<
                    NexusData,
                >,
            >(objects),
        ]
        .map(|tag| format!("{}::{}::{}", tag.address(), tag.module(), tag.name()))
    }

    fn assert_wrapper_filter(filter: &sui::grpc::EventFilter, expected: &[String; 2]) {
        let actual = filter
            .terms
            .iter()
            .map(|term| {
                let literal = &term.literals[0];
                let Some(Predicate::EventType(filter)) = literal.predicate.as_ref() else {
                    panic!("expected event type filter");
                };
                filter.event_type.clone().expect("event type path")
            })
            .collect::<Vec<_>>();

        assert_eq!(actual.as_slice(), expected.as_slice());
    }

    #[tokio::test]
    async fn subscription_filters_by_the_nexus_wrapper_types() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let expected_event_types = wrapper_event_types(&nexus_objects);
        let event = walk_event(&nexus_objects, 12);
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        sub_service_mock
            .expect_subscribe_events()
            .once()
            .returning(move |request| {
                let request = request.into_inner();
                assert_wrapper_filter(request.filter(), &expected_event_types);
                for path in [
                    "package_id",
                    "event_type",
                    "contents",
                    "checkpoint",
                    "transaction_digest",
                    "event_index",
                ] {
                    assert!(request
                        .read_mask
                        .as_ref()
                        .unwrap()
                        .paths
                        .iter()
                        .any(|item| item == path));
                }

                let frames = vec![
                    Ok(subscription_frame(None, watermark(b"live", None))),
                    Ok(subscription_frame(
                        Some(event.clone()),
                        watermark(b"event", Some(11)),
                    )),
                ];
                let stream = futures::stream::iter(frames).chain(futures::stream::pending());
                Ok(tonic::Response::new(
                    Box::pin(stream) as sui_mocks::grpc::BoxEventStream
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let poller = EventPoller::new(&rpc_url, nexus_objects);
        let mut pages = poller.start_polling(None).expect("poller should start");

        let progress = timeout(Duration::from_secs(2), pages.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(progress.checkpoint, 11);
        assert!(progress.events.is_empty());

        let event_page = timeout(Duration::from_secs(2), pages.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(event_page.checkpoint, 12);
        assert!(matches!(
            &event_page.events[0].data,
            NexusEventKind::WalkAdvanced(_)
        ));
    }

    #[tokio::test]
    async fn progress_frames_preserve_empty_event_pages() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        sub_service_mock
            .expect_subscribe_events()
            .once()
            .returning(move |_request| {
                let frames = vec![
                    Ok(subscription_frame(None, watermark(b"live", None))),
                    Ok(subscription_frame(None, watermark(b"tick", Some(9)))),
                ];
                let stream = futures::stream::iter(frames).chain(futures::stream::pending());
                Ok(tonic::Response::new(
                    Box::pin(stream) as sui_mocks::grpc::BoxEventStream
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let poller = EventPoller::new(&rpc_url, nexus_objects);
        let mut pages = poller.start_polling(None).expect("poller should start");
        let page = timeout(Duration::from_secs(2), pages.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        assert_eq!(page.checkpoint, 9);
        assert!(page.events.is_empty());
    }

    #[tokio::test]
    async fn replay_paginates_to_the_live_subscription_cursor() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let expected_event_types = wrapper_event_types(&nexus_objects);
        let event = walk_event(&nexus_objects, 8);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_list_events()
            .times(2)
            .returning({
                let calls = Arc::clone(&calls);
                move |request| {
                    let call = calls.fetch_add(1, Ordering::SeqCst);
                    let request = request.into_inner();
                    assert_eq!(request.start_checkpoint, Some(7));
                    assert_eq!(
                        request.options().before.as_deref(),
                        Some(b"live".as_slice())
                    );
                    assert_wrapper_filter(request.filter(), &expected_event_types);

                    let frames = if call == 0 {
                        assert!(request.options().after.is_none());
                        vec![Ok(list_frame(
                            Some(event.clone()),
                            watermark(b"page-one", Some(7)),
                            Some(sui::grpc::QueryEndReason::ItemLimit),
                        ))]
                    } else {
                        assert_eq!(
                            request.options().after.as_deref(),
                            Some(b"page-one".as_slice())
                        );
                        vec![Ok(list_frame(
                            None,
                            watermark(b"live", Some(8)),
                            Some(sui::grpc::QueryEndReason::CursorBound),
                        ))]
                    };

                    Ok(tonic::Response::new(
                        Box::pin(futures::stream::iter(frames))
                            as sui_mocks::grpc::BoxListEventsStream,
                    ))
                }
            });

        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        sub_service_mock
            .expect_subscribe_events()
            .once()
            .returning(move |_request| {
                let first = subscription_frame(None, watermark(b"live", None));
                let stream = futures::stream::iter([Ok(first)]).chain(futures::stream::pending());
                Ok(tonic::Response::new(
                    Box::pin(stream) as sui_mocks::grpc::BoxEventStream
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let poller = EventPoller::new(&rpc_url, nexus_objects);
        let mut pages = poller.start_polling(Some(7)).expect("poller should start");

        let progress = timeout(Duration::from_secs(2), pages.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(progress.checkpoint, 7);

        let event_page = timeout(Duration::from_secs(2), pages.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(event_page.checkpoint, 8);
        assert!(matches!(
            &event_page.events[0].data,
            NexusEventKind::WalkAdvanced(_)
        ));

        timeout(Duration::from_secs(2), async {
            while calls.load(Ordering::SeqCst) != 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("replay did not reach the live cursor");
    }

    #[tokio::test]
    async fn reconnect_replays_from_the_last_inclusive_checkpoint() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let live_event = walk_event(&nexus_objects, 10);
        let replayed_event = walk_event(&nexus_objects, 11);
        let subscription_calls = Arc::new(AtomicUsize::new(0));

        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        sub_service_mock
            .expect_subscribe_events()
            .times(2)
            .returning({
                let subscription_calls = Arc::clone(&subscription_calls);
                move |_request| {
                    let call = subscription_calls.fetch_add(1, Ordering::SeqCst);
                    let stream: sui_mocks::grpc::BoxEventStream = match call {
                        0 => Box::pin(futures::stream::iter([
                            Ok(subscription_frame(None, watermark(b"live-one", None))),
                            Ok(subscription_frame(
                                Some(live_event.clone()),
                                watermark(b"event-ten", Some(9)),
                            )),
                            Err(tonic::Status::unavailable("stream interrupted")),
                        ])),
                        1 => Box::pin(
                            futures::stream::iter([Ok(subscription_frame(
                                None,
                                watermark(b"live-two", None),
                            ))])
                            .chain(futures::stream::pending()),
                        ),
                        _ => panic!("unexpected subscription call"),
                    };

                    Ok(tonic::Response::new(stream))
                }
            });

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_list_events()
            .once()
            .returning(move |request| {
                let request = request.into_inner();
                assert_eq!(request.start_checkpoint, Some(10));
                assert_eq!(
                    request.options().before.as_deref(),
                    Some(b"live-two".as_slice())
                );

                let frames = [
                    Ok(list_frame(
                        Some(replayed_event.clone()),
                        watermark(b"event-eleven", Some(10)),
                        None,
                    )),
                    Ok(list_frame(
                        None,
                        watermark(b"live-two", Some(11)),
                        Some(sui::grpc::QueryEndReason::CursorBound),
                    )),
                ];
                Ok(tonic::Response::new(
                    Box::pin(futures::stream::iter(frames)) as sui_mocks::grpc::BoxListEventsStream,
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let poller = EventPoller::new(&rpc_url, nexus_objects);
        let mut pages = poller.start_polling(None).expect("poller should start");
        let mut event_checkpoints = Vec::new();

        timeout(Duration::from_secs(2), async {
            while event_checkpoints.len() != 2 {
                match pages
                    .recv()
                    .await
                    .expect("event channel should remain open")
                {
                    Ok(page) if !page.events.is_empty() => {
                        event_checkpoints.push(page.checkpoint);
                    }
                    Ok(_) | Err(_) => {}
                }
            }
        })
        .await
        .expect("reconnect did not replay the event gap");

        assert_eq!(event_checkpoints, [10, 11]);
        assert_eq!(subscription_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dropping_the_receiver_stops_reconnections() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let subscription_calls = Arc::new(AtomicUsize::new(0));
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        sub_service_mock.expect_subscribe_events().returning({
            let subscription_calls = Arc::clone(&subscription_calls);
            move |_request| {
                subscription_calls.fetch_add(1, Ordering::SeqCst);
                let frames = [Err(tonic::Status::unavailable("stream interrupted"))];
                Ok(tonic::Response::new(
                    Box::pin(futures::stream::iter(frames)) as sui_mocks::grpc::BoxEventStream,
                ))
            }
        });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let poller = EventPoller::new(&rpc_url, nexus_objects);
        let mut pages = poller.start_polling(None).expect("poller should start");

        let error = timeout(Duration::from_secs(2), pages.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(error.is_err());
        drop(pages);

        tokio::time::sleep(RECONNECT_DELAY * 4).await;
        assert_eq!(subscription_calls.load(Ordering::SeqCst), 1);
    }
}
