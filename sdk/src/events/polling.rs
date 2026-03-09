//! This module defines logic for event polling from Sui GRPC endpoints. This is
//! achieved by streaming checkpoints and subsequently batch-fetching
//! transactions with their events.
//!
//! See also: <https://github.com/Talus-Network/nexus/issues/724>

use {
    crate::{
        events::{FromSuiGrpcEvent, NexusEvent},
        sui,
        types::NexusObjects,
    },
    futures::TryStreamExt,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
    sui_rpc::field::FieldMaskUtil,
    thiserror::Error,
    tokio::sync::mpsc,
    tokio_util::sync::CancellationToken,
    tonic::Streaming,
};

#[derive(Debug, Error)]
pub enum PollerError {
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("GRPC error: {0}")]
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
    /// How many transactions should be fetched in a single batch. This is just
    /// a max value.
    transactions_batch_size: usize,
    /// This timeout defines max wait between transaction batch fetches. If this
    /// timeout is reached, the batch will be fetched even if the batch size is
    /// not reached, provided there is at least one transaction to fetch.
    transactions_batch_max_wait: Duration,
    cancellation_token: CancellationToken,
}

impl EventPoller {
    pub fn new(rpc_url: &str, nexus_objects: Arc<NexusObjects>) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            nexus_objects,
            channel_capacity: 100,
            transactions_batch_size: 10,
            transactions_batch_max_wait: Duration::from_millis(200),
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    pub fn with_transactions_batch_size(mut self, batch_size: usize) -> Self {
        self.transactions_batch_size = batch_size;
        self
    }

    pub fn with_transactions_batch_max_wait(mut self, max_wait: Duration) -> Self {
        self.transactions_batch_max_wait = max_wait;
        self
    }

    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.cancellation_token = cancellation_token;
        self
    }

    /// Start polling Nexus events and tasks from the given checkpoint sequence
    /// number. If the `from_checkpoint` is in the future, it is ignored.
    pub fn start_polling(
        self,
        mut from_checkpoint: Option<u64>,
    ) -> Result<mpsc::Receiver<Result<EventPage, PollerError>>, PollerError> {
        let this = Arc::new(self);

        let mut client = sui::grpc::Client::new(&this.rpc_url).map_err(|_| {
            PollerError::Configuration(format!("Invalid GRPC URL '{}'", this.rpc_url))
        })?;

        let (send_digest, next_digest) = mpsc::channel(this.transactions_batch_size * 2);
        let (send_page, next_page) = mpsc::channel(this.channel_capacity);

        // Spawn a task that accepts transaction digests via a channel and
        // fetches batches of transactions.
        tokio::spawn({
            let this = Arc::clone(&this);
            let send_page = send_page.clone();

            async move {
                this.fetch_transactions_and_notify(next_digest, send_page)
                    .await
            }
        });

        // Spawn a task that streams checkpoints and sends transaction digests
        // to the fetching task.
        tokio::spawn({
            let this = Arc::clone(&this);
            let send_page = send_page.clone();
            let send_digest = send_digest.clone();

            async move {
                'master: loop {
                    // First, start streaming checkpoints. This way we know how many
                    // checkpoints we need to fetch in the past.
                    let mut checkpoint_stream =
                        match this.start_streaming_checkpoints(&mut client).await {
                            Ok(stream) => stream,
                            Err(e) => {
                                if send_page.send(Err(e)).await.is_err() {
                                    break;
                                }

                                continue;
                            }
                        };

                    // If we need to catch up from the past.
                    if let Some(start_from) = from_checkpoint {
                        // Find the checkpoint we need to catch up to.
                        let checkpoint = match checkpoint_stream.try_next().await {
                            Ok(Some(response)) => response.checkpoint().clone(),
                            Ok(None) => {
                                if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Checkpoint stream ended unexpectedly while trying to find the starting checkpoint")))).await.is_err() {
                                    break;
                                }

                                continue;
                            }
                            Err(e) => {
                                if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Failed to receive checkpoint from stream while trying to find the starting checkpoint: {e}")))).await.is_err() {
                                    break;
                                }

                                continue;
                            }
                        };

                        // Fetch all the checkpoints that are between the requested
                        // starting checkpoint and the current one.
                        for checkpoint in start_from..checkpoint.sequence_number() {
                            let request = sui::grpc::GetCheckpointRequest::default()
                                .with_sequence_number(checkpoint)
                                .with_read_mask(sui::grpc::FieldMask::from_paths(&[
                                    "transactions.digest",
                                ]));

                            let mut ledger_client = client.ledger_client();

                            // Update the starting pointer. This way we can
                            // restart this whole process and continue where
                            // we left off.
                            from_checkpoint = Some(checkpoint);

                            tokio::select! {
                                _ = this.cancellation_token.cancelled() => {
                                    break 'master;
                                }

                                response = ledger_client.get_checkpoint(request) => {
                                    let response = match response {
                                        Ok(response) => response.into_inner(),
                                        Err(e) => {
                                            if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Failed to fetch checkpoint {checkpoint} while trying to catch up: {e}")))).await.is_err() {
                                                break 'master;
                                            }

                                            continue 'master;
                                        }
                                    };

                                    for tx in response.checkpoint().transactions() {
                                        if send_digest
                                            .send((checkpoint, tx.digest().to_string()))
                                            .await
                                            .is_err()
                                        {
                                            break 'master;
                                        }
                                    }
                                }
                            };
                        }

                        from_checkpoint = Some(checkpoint.sequence_number() + 1);

                        for tx in checkpoint.transactions() {
                            if send_digest
                                .send((checkpoint.sequence_number(), tx.digest().to_string()))
                                .await
                                .is_err()
                            {
                                break 'master;
                            }
                        }
                    }

                    // Finally we can just poll the stream.
                    loop {
                        tokio::select! {
                            _ = this.cancellation_token.cancelled() => {
                                break 'master;
                            }

                            response = checkpoint_stream.try_next() => {
                                let response = match response {
                                    Ok(Some(response)) => response,
                                    Ok(None) => {
                                        if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Checkpoint stream ended unexpectedly")))).await.is_err() {
                                            break 'master;
                                        }

                                        continue 'master;
                                    }
                                    Err(e) => {
                                        if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Failed to receive checkpoint from stream: {e}")))).await.is_err() {
                                            break 'master;
                                        }

                                        continue 'master;
                                    }
                                };

                                let checkpoint = response.checkpoint();

                                from_checkpoint = Some(checkpoint.sequence_number() + 1);

                                for tx in checkpoint.transactions() {
                                    if send_digest
                                        .send((checkpoint.sequence_number(), tx.digest().to_string()))
                                        .await
                                        .is_err()
                                    {
                                        break 'master;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(next_page)
    }

    /// Accept transaction digests via a channel and fetch batches of
    /// transactions of their events if the batch size is reached or the max
    /// wait between fetches is reached. Then notify about the fetched events
    /// via another channel.
    async fn fetch_transactions_and_notify(
        &self,
        mut next_digest: mpsc::Receiver<(u64, String)>,
        send_page: mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> Result<(), PollerError> {
        let mut client = sui::grpc::Client::new(&self.rpc_url).map_err(|_| {
            PollerError::Configuration(format!("Invalid GRPC URL '{}'", self.rpc_url))
        })?;

        let mut batch = Vec::with_capacity(self.transactions_batch_size);
        let mut last_fetched_at = Instant::now();

        loop {
            tokio::select! {
                _ = self.cancellation_token.cancelled() => {
                    break;
                }

                Some((checkpoint, digest)) = next_digest.recv() => {
                    batch.push(digest);

                    // Only fetch if the batch size is reached or if the max
                    // wait was exceeded.
                    if batch.len() < self.transactions_batch_size
                        && last_fetched_at.elapsed() < self.transactions_batch_max_wait
                    {
                        continue;
                    }

                    // Drain the batch and fetch the transactions and their
                    // events.
                    let digests = batch.drain(..).collect::<Vec<_>>();
                    let request = sui::grpc::BatchGetTransactionsRequest::default()
                        .with_digests(digests.clone())
                        .with_read_mask(sui::grpc::FieldMask::from_paths(&["events.events", "digest"]));

                    let response = match client
                        .ledger_client()
                        .batch_get_transactions(request)
                        .await {
                            Ok(response) => {
                                last_fetched_at = Instant::now();

                                response.into_inner()
                            },
                            Err(_) => {
                                if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Failed to fetch transactions for digests: {:?}", digests)))).await.is_err() {
                                    break;
                                }

                                // On fetch error, we return the digests back to
                                // the batch.
                                batch.extend(digests);

                                continue;
                            }
                        };

                    for transaction in response.transactions {
                        let transaction = transaction.transaction();

                        let Ok(events) = sui::types::TransactionEvents::try_from(transaction.events())
                        else {
                            continue;
                        };

                        let nexus_events = events.0.iter().enumerate().filter_map(|(index, event)| {
                            NexusEvent::from_sui_grpc_event(
                                index as u64,
                                transaction.digest().parse().ok()?,
                                event,
                                &self.nexus_objects,
                            )
                            .inspect_err(|e| {
                                println!("Failed to convert Sui GRPC event to Nexus event: {e}");
                            })
                            .ok()
                        });

                        if send_page.send(Ok(EventPage {
                            events: nexus_events.collect(),
                            checkpoint,
                        })).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Helper to start the streaming of checkpoints from GRPC.
    async fn start_streaming_checkpoints(
        &self,
        client: &mut sui::grpc::Client,
    ) -> Result<Streaming<sui::grpc::SubscribeCheckpointsResponse>, PollerError> {
        let request = sui::grpc::SubscribeCheckpointsRequest::default().with_read_mask(
            sui::grpc::FieldMask::from_paths(&["transactions.digest", "sequence_number"]),
        );

        client
            .subscription_client()
            .subscribe_checkpoints(request)
            .await
            .map_err(|e| {
                PollerError::Rpc(anyhow::anyhow!("Failed to subscribe to checkpoints: {e}"))
            })
            .map(|response| response.into_inner())
    }
}

#[cfg(test)]
mod tests {
    // TODO:
}
