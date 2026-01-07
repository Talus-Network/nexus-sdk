//! This module wraps a Sui GQL endpoint to provide the functionality to fetch
//! events. These are sent over a channel to the consumer. ALl events are parsed
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
    pub fn poll_nexus_events(
        &self,
        from_cursor: Option<String>,
        from_checkpoint: Option<u64>,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::mpsc::Receiver<EventPage>,
    ) {
        let (notify_about_events, next_page) =
            tokio::sync::mpsc::channel::<EventPage>(self.channel_capacity);

        let poller = {
            let nexus_objects = self.nexus_objects.clone();
            let url = self.url.clone();
            let mut cursor = from_cursor;
            let mut at_checkpoint = from_checkpoint;

            // Polling intervals.
            let mut poll_interval = tokio::time::Duration::from_millis(100);
            let max_poll_interval = self.max_poll_interval;

            // NOTE: that the poller process is infallible. RPC errors result
            // in backoff and retry. Events that cannot be parsed are ignored.
            tokio::spawn(async move {
                let event_wrapper = format!(
                    "{package}::{module}::{name}",
                    package = nexus_objects.primitives_pkg_id,
                    module = primitives::Event::EVENT_WRAPPER.module.as_str(),
                    name = primitives::Event::EVENT_WRAPPER.name.as_str(),
                );

                loop {
                    let request = events_query::Variables {
                        after: cursor.clone(),
                        at_checkpoint,
                        filter: events_query::EventFilter {
                            type_: Some(event_wrapper.clone()),
                            sender: None,
                            after_checkpoint: None,
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
                        Err(_) => {
                            tokio::time::sleep(poll_interval).await;

                            poll_interval = std::cmp::min(poll_interval * 2, max_poll_interval);

                            continue;
                        }
                    };

                    // Parse the GQL response.
                    let response: Response<events_query::ResponseData> = match response.json().await
                    {
                        Ok(data) => data,
                        Err(_) => {
                            tokio::time::sleep(poll_interval).await;

                            poll_interval = std::cmp::min(poll_interval * 2, max_poll_interval);

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
                            tokio::time::sleep(poll_interval).await;

                            poll_interval = std::cmp::min(poll_interval * 2, max_poll_interval);

                            continue;
                        }
                    };

                    // If there are no events, backoff.
                    if events.is_empty() {
                        tokio::time::sleep(poll_interval).await;

                        poll_interval = std::cmp::min(poll_interval * 2, max_poll_interval);

                        continue;
                    }

                    // `page_info.end_cursor` should always exist.
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
                    at_checkpoint = None;

                    match notify_about_events
                        .send(EventPage {
                            events: nexus_events.collect(),
                            next_cursor,
                        })
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

        let (_poller, mut receiver) = fetcher.poll_nexus_events(None, None);

        if let Some(page) = receiver.recv().await {
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
}
