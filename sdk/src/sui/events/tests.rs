use {
    super::{driver::RECONNECT_DELAY, *},
    crate::{sui, test_utils::sui_mocks},
    futures::StreamExt as _,
    std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    sui_rpc::field::FieldMaskUtil as _,
    tokio::time::{timeout, Duration},
    tokio_util::sync::CancellationToken,
};

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
    end: sui::grpc::QueryEndReason,
) -> sui::grpc::ListEventsResponse {
    let mut response = sui::grpc::ListEventsResponse::default();
    if let Some(event) = event {
        response.set_event(event);
    }
    response.set_watermark(watermark);
    let mut query_end = sui::grpc::QueryEnd::default();
    query_end.set_reason(end);
    response.set_end(query_end);
    response
}

#[test]
fn zero_channel_capacity_is_a_configuration_error() {
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::default(),
    );
    let result = EventIngestor::new("http://127.0.0.1:1", query)
        .with_channel_capacity(0)
        .start(None);

    match result {
        Err(EventIngestionError::Configuration(message)) => {
            assert!(message.contains("channel capacity"));
        }
        Err(other) => {
            panic!("expected configuration failure, found {other:?}");
        }
        Ok(_) => panic!("zero channel capacity was accepted"),
    }
}

#[test]
fn invalid_event_field_is_a_configuration_error() {
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::from_paths(["not_an_event_field"]),
    );
    let result = EventIngestor::new("http://127.0.0.1:1", query).start(None);

    match result {
        Err(EventIngestionError::Configuration(message)) => {
            assert!(message.contains("not_an_event_field"));
        }
        Err(other) => {
            panic!("expected configuration failure, found {other:?}");
        }
        Ok(_) => panic!("invalid event field was accepted"),
    }
}

#[tokio::test]
async fn ingestor_adds_engine_fields_and_returns_raw_events() {
    let mut event = sui::grpc::Event::default();
    event.set_contents(vec![1, 2, 3]);
    event.set_checkpoint(12);
    event.set_transaction_digest(sui::types::Digest::ZERO);
    event.set_event_index(4);

    let expected_event = event.clone();
    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription
        .expect_subscribe_events()
        .once()
        .returning(move |request| {
            let request = request.into_inner();
            let paths = &request.read_mask.as_ref().unwrap().paths;
            for path in [
                "contents",
                "checkpoint",
                "transaction_digest",
                "event_index",
            ] {
                assert!(paths.iter().any(|candidate| candidate == path));
            }

            let frames = [
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
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::from_paths(["contents"]),
    );
    let mut pages = EventIngestor::new(&rpc_url, query)
        .start(None)
        .expect("ingestor should start");

    let progress = timeout(Duration::from_secs(2), pages.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(progress.checkpoint, 11);
    assert!(progress.events.is_empty());

    let page = timeout(Duration::from_secs(2), pages.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(page.checkpoint, 12);
    assert_eq!(page.events, [expected_event]);
}

#[tokio::test]
async fn replay_uses_the_live_query_and_stops_at_its_cursor() {
    let mut event = sui::grpc::Event::default();
    event.set_contents(vec![4, 5, 6]);
    event.set_checkpoint(8);
    event.set_transaction_digest(sui::types::Digest::ZERO);
    event.set_event_index(2);
    let expected_event = event.clone();

    let filter = sui::grpc::EventFilter::default();
    let expected_filter = filter.clone();
    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription
        .expect_subscribe_events()
        .once()
        .returning(move |_request| {
            let first = subscription_frame(None, watermark(b"live", None));
            let stream = futures::stream::iter([Ok(first)]).chain(futures::stream::pending());
            Ok(tonic::Response::new(
                Box::pin(stream) as sui_mocks::grpc::BoxEventStream
            ))
        });

    let mut ledger = sui_mocks::grpc::MockLedgerService::new();
    ledger
        .expect_list_events()
        .once()
        .returning(move |request| {
            let request = request.into_inner();
            assert_eq!(request.start_checkpoint, Some(7));
            assert_eq!(
                request.options().before.as_deref(),
                Some(b"live".as_slice())
            );
            assert_eq!(request.filter(), &expected_filter);
            let paths = &request.read_mask.as_ref().unwrap().paths;
            for path in [
                "contents",
                "checkpoint",
                "transaction_digest",
                "event_index",
            ] {
                assert!(paths.iter().any(|candidate| candidate == path));
            }

            let frame = list_frame(
                Some(event.clone()),
                watermark(b"live", Some(8)),
                sui::grpc::QueryEndReason::CursorBound,
            );
            Ok(tonic::Response::new(
                Box::pin(futures::stream::iter([Ok(frame)]))
                    as sui_mocks::grpc::BoxListEventsStream,
            ))
        });

    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let query = RawEventQuery::new(filter, sui::grpc::FieldMask::from_paths(["contents"]));
    let mut pages = EventIngestor::new(&rpc_url, query)
        .start(Some(7))
        .expect("ingestor should start");

    let page = timeout(Duration::from_secs(2), async {
        loop {
            let page = pages.recv().await.unwrap().unwrap();
            if !page.events.is_empty() {
                return page;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(page.checkpoint, 8);
    assert_eq!(page.events, [expected_event]);
}

#[tokio::test]
async fn replay_continues_after_an_item_limit() {
    let mut event = sui::grpc::Event::default();
    event.set_checkpoint(8);
    event.set_transaction_digest(sui::types::Digest::ZERO);
    event.set_event_index(2);

    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription
        .expect_subscribe_events()
        .once()
        .returning(move |_request| {
            let first = subscription_frame(None, watermark(b"live", None));
            let stream = futures::stream::iter([Ok(first)]).chain(futures::stream::pending());
            Ok(tonic::Response::new(
                Box::pin(stream) as sui_mocks::grpc::BoxEventStream
            ))
        });

    let calls = Arc::new(AtomicUsize::new(0));
    let mut ledger = sui_mocks::grpc::MockLedgerService::new();
    ledger.expect_list_events().times(2).returning({
        let calls = Arc::clone(&calls);
        move |request| {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            let request = request.into_inner();
            assert_eq!(request.start_checkpoint, Some(7));
            assert_eq!(
                request.options().before.as_deref(),
                Some(b"live".as_slice())
            );

            let frame = if call == 0 {
                assert!(request.options().after.is_none());
                list_frame(
                    Some(event.clone()),
                    watermark(b"page-one", Some(7)),
                    sui::grpc::QueryEndReason::ItemLimit,
                )
            } else {
                assert_eq!(
                    request.options().after.as_deref(),
                    Some(b"page-one".as_slice())
                );
                list_frame(
                    None,
                    watermark(b"live", Some(8)),
                    sui::grpc::QueryEndReason::CursorBound,
                )
            };
            Ok(tonic::Response::new(
                Box::pin(futures::stream::iter([Ok(frame)]))
                    as sui_mocks::grpc::BoxListEventsStream,
            ))
        }
    });

    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::default(),
    );
    let mut pages = EventIngestor::new(&rpc_url, query)
        .start(Some(7))
        .expect("ingestor should start");

    timeout(Duration::from_secs(2), async {
        loop {
            let page = pages.recv().await.unwrap().unwrap();
            if !page.events.is_empty() {
                return;
            }
        }
    })
    .await
    .expect("replay did not emit the event");

    timeout(Duration::from_secs(2), async {
        while calls.load(Ordering::SeqCst) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("replay did not continue after the item limit");
}

#[tokio::test]
async fn reconnect_replays_from_the_last_inclusive_checkpoint() {
    let mut first_event = sui::grpc::Event::default();
    first_event.set_checkpoint(10);
    first_event.set_transaction_digest(sui::types::Digest::ZERO);
    first_event.set_event_index(1);

    let mut replayed_event = sui::grpc::Event::default();
    replayed_event.set_checkpoint(11);
    replayed_event.set_transaction_digest(sui::types::Digest::ZERO);
    replayed_event.set_event_index(2);

    let subscription_calls = Arc::new(AtomicUsize::new(0));
    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription.expect_subscribe_events().times(2).returning({
        let subscription_calls = Arc::clone(&subscription_calls);
        move |_request| {
            let call = subscription_calls.fetch_add(1, Ordering::SeqCst);
            let stream: sui_mocks::grpc::BoxEventStream = match call {
                0 => Box::pin(futures::stream::iter([
                    Ok(subscription_frame(None, watermark(b"live-one", None))),
                    Ok(subscription_frame(
                        Some(first_event.clone()),
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

    let mut ledger = sui_mocks::grpc::MockLedgerService::new();
    ledger
        .expect_list_events()
        .once()
        .returning(move |request| {
            let request = request.into_inner();
            assert_eq!(request.start_checkpoint, Some(10));
            assert_eq!(
                request.options().before.as_deref(),
                Some(b"live-two".as_slice())
            );

            let frame = list_frame(
                Some(replayed_event.clone()),
                watermark(b"live-two", Some(11)),
                sui::grpc::QueryEndReason::CursorBound,
            );
            Ok(tonic::Response::new(
                Box::pin(futures::stream::iter([Ok(frame)]))
                    as sui_mocks::grpc::BoxListEventsStream,
            ))
        });

    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::default(),
    );
    let mut pages = EventIngestor::new(&rpc_url, query)
        .start(None)
        .expect("ingestor should start");
    let mut checkpoints = Vec::new();

    timeout(Duration::from_secs(2), async {
        while checkpoints.len() != 2 {
            match pages.recv().await.unwrap() {
                Ok(page) if !page.events.is_empty() => {
                    checkpoints.push(page.checkpoint);
                }
                Ok(_) | Err(_) => {}
            }
        }
    })
    .await
    .expect("ingestor did not replay after reconnecting");

    assert_eq!(checkpoints, [10, 11]);
    assert_eq!(subscription_calls.load(Ordering::SeqCst), 2);
}

struct FailingQuery {
    decode_calls: Arc<AtomicUsize>,
}

impl EventQuery for FailingQuery {
    type Error = std::io::Error;
    type Output = ();

    fn filter(&self) -> sui::grpc::EventFilter {
        sui::grpc::EventFilter::default()
    }

    fn read_mask(&self) -> sui::grpc::FieldMask {
        sui::grpc::FieldMask::default()
    }

    fn decode(&self, _event: sui::grpc::Event) -> Result<Option<Self::Output>, Self::Error> {
        self.decode_calls.fetch_add(1, Ordering::SeqCst);
        Err(std::io::Error::other("unsupported event shape"))
    }
}

#[tokio::test]
async fn decode_failure_is_terminal_and_preserves_event_identity() {
    let mut event = sui::grpc::Event::default();
    event.set_checkpoint(10);
    event.set_transaction_digest(sui::types::Digest::ZERO);
    event.set_event_index(3);

    let subscription_calls = Arc::new(AtomicUsize::new(0));
    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription.expect_subscribe_events().once().returning({
        let subscription_calls = Arc::clone(&subscription_calls);
        move |_request| {
            subscription_calls.fetch_add(1, Ordering::SeqCst);
            let frames = [
                Ok(subscription_frame(None, watermark(b"live", None))),
                Ok(subscription_frame(
                    Some(event.clone()),
                    watermark(b"event", Some(9)),
                )),
            ];
            Ok(tonic::Response::new(
                Box::pin(futures::stream::iter(frames)) as sui_mocks::grpc::BoxEventStream,
            ))
        }
    });

    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let decode_calls = Arc::new(AtomicUsize::new(0));
    let query = FailingQuery {
        decode_calls: Arc::clone(&decode_calls),
    };
    let mut pages = EventIngestor::new(&rpc_url, query)
        .start(None)
        .expect("ingestor should start");

    let progress = pages.recv().await.unwrap().unwrap();
    assert_eq!(progress.checkpoint, 9);

    let error = pages.recv().await.unwrap().unwrap_err();
    match error {
        EventIngestionError::Decode {
            checkpoint,
            transaction_digest,
            event_index,
            source,
        } => {
            assert_eq!(checkpoint, 10);
            assert_eq!(transaction_digest, sui::types::Digest::ZERO.to_string());
            assert_eq!(event_index, 3);
            assert_eq!(source.to_string(), "unsupported event shape");
        }
        other => panic!("expected decode failure, found {other:?}"),
    }

    assert!(pages.recv().await.is_none());
    assert_eq!(decode_calls.load(Ordering::SeqCst), 1);
    assert_eq!(subscription_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn permanent_rpc_failure_is_terminal() {
    let subscription_calls = Arc::new(AtomicUsize::new(0));
    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription.expect_subscribe_events().returning({
        let subscription_calls = Arc::clone(&subscription_calls);
        move |_request| {
            subscription_calls.fetch_add(1, Ordering::SeqCst);
            Err(tonic::Status::invalid_argument("query was rejected"))
        }
    });

    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::default(),
    );
    let mut pages = EventIngestor::new(&rpc_url, query)
        .start(None)
        .expect("ingestor should start");

    let error = pages.recv().await.unwrap().unwrap_err();
    assert!(error.to_string().contains("query was rejected"));
    assert!(pages.recv().await.is_none());
    assert_eq!(subscription_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_stops_blocked_error_delivery() {
    let subscription_calls = Arc::new(AtomicUsize::new(0));
    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription.expect_subscribe_events().returning({
        let subscription_calls = Arc::clone(&subscription_calls);
        move |_request| {
            subscription_calls.fetch_add(1, Ordering::SeqCst);
            Err(tonic::Status::unavailable("stream interrupted"))
        }
    });

    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let cancellation_token = CancellationToken::new();
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::default(),
    );
    let mut pages = EventIngestor::new(&rpc_url, query)
        .with_channel_capacity(1)
        .with_cancellation_token(cancellation_token.clone())
        .start(None)
        .expect("ingestor should start");

    timeout(Duration::from_secs(2), async {
        while subscription_calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("ingestor did not attempt to reconnect");
    tokio::time::sleep(RECONNECT_DELAY * 2).await;
    cancellation_token.cancel();
    timeout(Duration::from_secs(2), async {
        while !pages.is_closed() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancellation did not stop error delivery");

    assert!(pages.recv().await.unwrap().is_err());
    assert!(pages.recv().await.is_none());
}

#[tokio::test]
async fn dropping_the_receiver_stops_reconnection() {
    let subscription_calls = Arc::new(AtomicUsize::new(0));
    let mut subscription = sui_mocks::grpc::MockSubscriptionService::new();
    subscription.expect_subscribe_events().returning({
        let subscription_calls = Arc::clone(&subscription_calls);
        move |_request| {
            subscription_calls.fetch_add(1, Ordering::SeqCst);
            Ok(tonic::Response::new(Box::pin(futures::stream::iter([Err(
                tonic::Status::unavailable("stream interrupted"),
            )]))
                as sui_mocks::grpc::BoxEventStream))
        }
    });

    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let query = RawEventQuery::new(
        sui::grpc::EventFilter::default(),
        sui::grpc::FieldMask::default(),
    );
    let mut pages = EventIngestor::new(&rpc_url, query)
        .start(None)
        .expect("ingestor should start");

    assert!(pages.recv().await.unwrap().is_err());
    drop(pages);
    tokio::time::sleep(RECONNECT_DELAY * 4).await;

    assert_eq!(subscription_calls.load(Ordering::SeqCst), 1);
}
