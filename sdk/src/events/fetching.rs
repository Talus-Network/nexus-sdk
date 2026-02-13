//! This module wraps a Sui GQL endpoint to provide the functionality to fetch
//! events. These are sent over a channel to the consumer. All events are parsed
//! to [`NexusEvent`]. Those that cannot be parsed are ignored.
//!
//! Note that the polling will stop whenever the receiver side of the channel is
//! dropped.

use {
    crate::{
        events::{
            graphql::events_query::{events_query, EventsQuery},
            FromSuiGqlEvent,
            NexusEvent,
        },
        idents::primitives,
        sui,
        types::NexusObjects,
    },
    graphql_client::{GraphQLQuery, Response},
    reqwest::Client,
    std::sync::Arc,
};

/// This struct defines the structure of the message sent over the channel.
pub struct EventPage {
    pub events: Vec<NexusEvent>,
    pub next_cursor: String,
}

/// The fetcher struct itself. Responsible for creating a poller thread,
/// fetching and parsing events and sending them over a channel.
#[derive(Clone)]
pub struct EventFetcher {
    url: String,
    channel_capacity: usize,
    max_poll_interval: tokio::time::Duration,
    nexus_objects: Arc<NexusObjects>,
}

impl EventFetcher {
    /// Create a new EventFetcher instance pointing to the given graphql URL.
    pub fn new(url: &str, nexus_objects: Arc<NexusObjects>) -> Self {
        Self {
            url: url.to_string(),
            channel_capacity: 100,
            max_poll_interval: tokio::time::Duration::from_secs(2),
            nexus_objects,
        }
    }

    /// Set the desired channel capacity.
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Set the maximum polling interval.
    pub fn with_max_poll_interval(mut self, interval: tokio::time::Duration) -> Self {
        self.max_poll_interval = interval;
        self
    }

    /// Start polling for events and sending them over the given channel. Return
    /// the JoinHandle of the polling task as well as the channel receiver.
    ///
    /// The optional `inner_type` parameter allows filtering by a specific event
    /// type wrapped in `EventWrapper<T>`. When `None`, all `EventWrapper<*>`
    /// events are fetched. When `Some(type_tag)`, only events matching
    /// `EventWrapper<type_tag>` are returned by the GraphQL query.
    ///
    /// This is useful for scenarios where multiple deployments share the same
    /// `primitives_pkg_id` but have different `workflow_pkg_id` values, as it
    /// allows precise filtering at the GraphQL level rather than client-side.
    pub fn poll_nexus_events(
        &self,
        from_cursor: Option<String>,
        from_checkpoint: Option<u64>,
        inner_type: Option<sui::types::TypeTag>,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::mpsc::Receiver<anyhow::Result<EventPage>>,
    ) {
        let (notify_about_events, next_page) =
            tokio::sync::mpsc::channel::<anyhow::Result<EventPage>>(self.channel_capacity);

        let poller = {
            let nexus_objects = self.nexus_objects.clone();
            let url = self.url.clone();
            let mut cursor = from_cursor;
            let after_checkpoint = from_checkpoint;

            // Polling intervals.
            let mut poll_interval = tokio::time::Duration::from_millis(100);
            let max_poll_interval = self.max_poll_interval;

            // NOTE: that the poller process is infallible. RPC errors result
            // in backoff and retry. Events that cannot be parsed are ignored.
            tokio::spawn(async move {
                let event_wrapper = sui::types::StructTag::new(
                    nexus_objects.primitives_pkg_id,
                    primitives::Event::EVENT_WRAPPER.module,
                    primitives::Event::EVENT_WRAPPER.name,
                    inner_type.map(|t| vec![t]).unwrap_or_default(),
                );

                loop {
                    let request = events_query::Variables {
                        after: cursor.clone(),
                        filter: events_query::EventFilter {
                            type_: Some(event_wrapper.to_string()),
                            sender: None,
                            after_checkpoint,
                            at_checkpoint: None,
                            before_checkpoint: None,
                            module: None,
                        },
                    };

                    let query = EventsQuery::build_query(request);
                    let client = Client::new();

                    // Send the GQL request.
                    let response = match client.post(&url).json(&query).send().await {
                        Ok(resp) => resp,
                        Err(e) => {
                            if notify_about_events
                                .send(Err(anyhow::anyhow!("Failed to send GQL request: {e}")))
                                .await
                                .is_err()
                            {
                                // Receiver dropped, exit the poller process.
                                break;
                            }

                            Self::sleep_and_backoff(&mut poll_interval, max_poll_interval).await;

                            continue;
                        }
                    };

                    // Parse the GQL response.
                    let response: Response<events_query::ResponseData> = match response.json().await
                    {
                        Ok(data) => data,
                        Err(e) => {
                            if notify_about_events
                                .send(Err(anyhow::anyhow!("Failed to send GQL request: {e}")))
                                .await
                                .is_err()
                            {
                                // Receiver dropped, exit the poller process.
                                break;
                            }

                            Self::sleep_and_backoff(&mut poll_interval, max_poll_interval).await;

                            continue;
                        }
                    };

                    // Parse the response data.
                    let (events, page_info) = match response
                        .data
                        .and_then(|d| d.events)
                        .map(|e| (e.nodes, e.page_info))
                    {
                        Some((events, page_info)) => (events, page_info),
                        None => {
                            Self::sleep_and_backoff(&mut poll_interval, max_poll_interval).await;

                            continue;
                        }
                    };

                    // If there are no events, backoff.
                    if events.is_empty() {
                        Self::sleep_and_backoff(&mut poll_interval, max_poll_interval).await;

                        continue;
                    }

                    // Cursor is only empty when there are no events. Meaning
                    // this should never be reached.
                    let Some(next_cursor) = page_info.end_cursor.clone() else {
                        continue;
                    };

                    let nexus_events = events.into_iter().filter_map(|event| {
                        let index = event.sequence_number;
                        let digest = event.transaction.and_then(|t| t.digest.parse().ok())?;
                        let package_id = event.transaction_module?.package?.address;
                        let contents = event.contents?;

                        NexusEvent::from_sui_gql_event(
                            index,
                            digest,
                            package_id,
                            &contents,
                            &nexus_objects,
                        )
                        .ok()
                    });

                    cursor = Some(next_cursor.clone());

                    match notify_about_events
                        .send(Ok(EventPage {
                            events: nexus_events.collect(),
                            next_cursor,
                        }))
                        .await
                    {
                        Ok(()) => {
                            poll_interval = tokio::time::Duration::from_millis(100);

                            tokio::time::sleep(poll_interval).await;
                        }
                        Err(_) => {
                            // Receiver dropped, exit the poller process.
                            break;
                        }
                    }
                }
            })
        };

        (poller, next_page)
    }

    async fn sleep_and_backoff(
        current_interval: &mut tokio::time::Duration,
        max_interval: tokio::time::Duration,
    ) {
        tokio::time::sleep(*current_interval).await;

        *current_interval = std::cmp::min(*current_interval * 2, max_interval);
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::{DAGCreatedEvent, NexusEventKind},
            sui,
            test_utils::sui_mocks,
        },
        mockito::Server,
    };

    #[tokio::test]
    async fn test_event_fetcher_polling() {
        let mut rng = rand::thread_rng();
        let mut server = Server::new_async().await;
        let objects = sui_mocks::mock_nexus_objects();
        let primitives_pkg_id = objects.primitives_pkg_id;

        let dag_id_1 = sui::types::Address::generate(&mut rng);
        let dag_id_2 = sui::types::Address::generate(&mut rng);
        let dag_id_3 = sui::types::Address::generate(&mut rng);
        let events = vec![
            NexusEventKind::DAGCreated(DAGCreatedEvent { dag: dag_id_1 }),
            NexusEventKind::DAGCreated(DAGCreatedEvent { dag: dag_id_2 }),
            NexusEventKind::DAGCreated(DAGCreatedEvent { dag: dag_id_3 }),
        ];
        let digest = sui::types::Digest::generate(&mut rng);

        let mock = sui_mocks::gql::mock_event_query(
            &mut server,
            primitives_pkg_id,
            events.clone(),
            Some(digest),
            Some("12345"),
        );

        let fetcher = EventFetcher::new(&format!("{}/graphql", &server.url()), Arc::new(objects));

        let (_poller, mut receiver) = fetcher.poll_nexus_events(None, None, None);

        if let Some(Ok(page)) = receiver.recv().await {
            assert_eq!(page.next_cursor, "12345".to_string());
            assert_eq!(page.events.len(), 3);

            let first_event = &page.events[0];
            assert_eq!(first_event.id, (digest, 0));
            assert!(matches!(
                first_event.data,
                NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_id_1
            ));

            let second_event = &page.events[1];
            assert_eq!(second_event.id, (digest, 1));
            assert!(matches!(
                second_event.data,
                NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_id_2
            ));

            let third_event = &page.events[2];
            assert_eq!(third_event.id, (digest, 2));
            assert!(matches!(
                third_event.data,
                NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_id_3
            ));
        } else {
            panic!("Did not receive any events from the fetcher.");
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_event_fetcher_wrong_url() {
        let objects = sui_mocks::mock_nexus_objects();
        let fetcher = EventFetcher::new("http://invalid.url", Arc::new(objects));

        let (_poller, mut receiver) = fetcher.poll_nexus_events(None, None, None);

        if let Some(result) = receiver.recv().await {
            assert!(result.is_err());
            assert!(
                matches!(result, Err(e) if e.to_string().contains("Failed to send GQL request"))
            );
        } else {
            panic!("Did not receive any events from the fetcher.");
        }
    }
}
