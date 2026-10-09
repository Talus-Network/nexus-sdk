#![cfg(feature = "test_utils")]

use {
    futures::StreamExt as _,
    nexus_sdk::{
        move_bindings::{self, scheduler::task::Task, workflow::execution::DAGExecution},
        nexus::recovery::{RecoveryReader, RecoveryWindow},
        sui,
        test_utils::sui_mocks::{
            self,
            grpc::{self, MockLedgerService},
        },
    },
    std::{
        collections::BTreeSet,
        num::NonZeroUsize,
        sync::{Arc, Mutex},
        time::Duration,
    },
};

fn reader(rpc: &str) -> RecoveryReader<'_> {
    RecoveryReader::new(rpc).unwrap()
}

#[tokio::test(start_paused = true)]
async fn stalled_read_returns_a_typed_deadline_observation() {
    let error = reader("http://127.0.0.1:1")
        .read(
            "reading chain state",
            std::future::pending::<anyhow::Result<()>>,
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<tonic::Status>().unwrap().code(),
        tonic::Code::DeadlineExceeded
    );
}

async fn discover(rpc: &str) -> anyhow::Result<nexus_sdk::nexus::recovery::WorkObjects> {
    reader(rpc)
        .discover_work_objects(
            &sui_mocks::mock_nexus_context(),
            RecoveryWindow {
                start: 10,
                tip: 20,
                timestamp_ms: 0,
            },
            NonZeroUsize::MIN,
        )
        .await
}

fn id(number: u64) -> sui::types::Address {
    let mut bytes = [0; 32];
    bytes[24..].copy_from_slice(&number.to_be_bytes());
    sui::types::Address::new(bytes)
}

fn frame(
    checkpoint: u64,
    ids: &[u64],
    cursor: &[u8],
    end: Option<sui::grpc::QueryEndReason>,
) -> sui::grpc::ListTransactionsResponse {
    let mut response = sui::grpc::ListTransactionsResponse::default();
    response.set_watermark(sui::grpc::Watermark::default().with_cursor(cursor.to_vec()));
    if !ids.is_empty() {
        let objects = ids
            .iter()
            .map(|number| {
                sui::grpc::ChangedObject::default()
                    .with_object_id(id(*number).to_string())
                    .with_output_owner(
                        sui::grpc::Owner::default().with_kind(sui::grpc::owner::OwnerKind::Shared),
                    )
            })
            .collect();
        response.set_transaction(
            sui::grpc::ExecutedTransaction::default()
                .with_checkpoint(checkpoint)
                .with_effects(
                    sui::grpc::TransactionEffects::default().with_changed_objects(objects),
                ),
        );
    }
    if let Some(reason) = end {
        response.set_end(sui::grpc::QueryEnd::default().with_reason(reason));
    }
    response
}

fn stream(
    frames: Vec<sui::grpc::ListTransactionsResponse>,
) -> tonic::Response<grpc::BoxListTransactionsStream> {
    tonic::Response::new(Box::pin(futures::stream::iter(frames.into_iter().map(Ok))))
}

fn metadata(
    ledger: &mut MockLedgerService,
    expected: BTreeSet<sui::types::Address>,
) -> Arc<Mutex<BTreeSet<sui::types::Address>>> {
    let observed = Arc::new(Mutex::new(BTreeSet::new()));
    let context = sui_mocks::mock_nexus_context();
    let task_type = move_bindings::struct_tag::<Task>(&context).to_string();
    let execution_type = move_bindings::struct_tag::<DAGExecution>(&context).to_string();
    let seen = Arc::clone(&observed);
    ledger.expect_batch_get_objects().returning(move |request| {
        let request = request.into_inner();
        assert!(request.requests.len() <= 1_000);
        let objects = request
            .requests
            .into_iter()
            .map(|request| {
                assert!(
                    request.version.is_none(),
                    "Recovery requested an old object version"
                );
                let address: sui::types::Address =
                    request.object_id.as_ref().unwrap().parse().unwrap();
                assert!(expected.contains(&address));
                assert!(
                    seen.lock().unwrap().insert(address),
                    "Metadata was read twice"
                );
                let mut result = sui::grpc::GetObjectResult::default();
                result.result = Some(sui::grpc::get_object_result::Result::Object(
                    sui::grpc::Object::default()
                        .with_object_id(address.to_string())
                        .with_object_type(if address.as_bytes()[31].is_multiple_of(2) {
                            task_type.clone()
                        } else {
                            execution_type.clone()
                        }),
                ));
                result
            })
            .collect();
        Ok(tonic::Response::new(
            sui::grpc::BatchGetObjectsResponse::default().with_objects(objects),
        ))
    });
    ledger.expect_get_transaction().never();
    ledger.expect_get_object().never();
    observed
}

#[tokio::test]
async fn discovery_resumes_opaque_cursors_and_reads_only_unique_current_objects() {
    use sui::grpc::QueryEndReason::{CheckpointBound, ItemLimit, ScanLimit};
    let mut ledger = MockLedgerService::new();
    let mut sequence = mockall::Sequence::new();
    let cases = [
        (
            None,
            vec![
                frame(10, &[2, 3, 2], b"item", None),
                frame(0, &[], b"scanned", Some(ScanLimit)),
            ],
        ),
        (
            Some(b"scanned".as_slice()),
            vec![frame(13, &[2], b"limited", Some(ItemLimit))],
        ),
        (
            Some(b"limited".as_slice()),
            vec![
                frame(20, &[3, 4], b"last", None),
                frame(0, &[], b"end", Some(CheckpointBound)),
            ],
        ),
    ];
    for (after, frames) in cases {
        ledger
            .expect_list_transactions()
            .once()
            .in_sequence(&mut sequence)
            .return_once(move |request| {
                let request = request.into_inner();
                assert_eq!(request.start_checkpoint, Some(10));
                assert_eq!(request.end_checkpoint, Some(21));
                let options = request.options.unwrap();
                assert_eq!(options.limit, None);
                assert_eq!(options.after.as_deref(), after);
                let mask = request.read_mask.unwrap().paths;
                assert!(mask
                    .iter()
                    .all(|path| !path.starts_with("objects") && !path.starts_with("events")));
                let filter = format!("{:?}", request.filter.unwrap());
                assert!(filter.contains("a1::event::EventWrapper"));
                assert!(filter.contains("a2::distributed_event::DistributedEventWrapper"));
                Ok(stream(frames))
            });
    }
    let observed = metadata(&mut ledger, [id(2), id(3), id(4)].into());
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let objects = reader(&rpc)
        .discover_work_objects(
            &sui_mocks::mock_nexus_context(),
            RecoveryWindow {
                start: 10,
                tip: 20,
                timestamp_ms: 0,
            },
            NonZeroUsize::MIN,
        )
        .await
        .unwrap();
    assert_eq!(objects.tasks, [id(2), id(4)].into());
    assert_eq!(objects.executions, [id(3)].into());
    assert_eq!(*observed.lock().unwrap(), [id(2), id(3), id(4)].into());
}

fn clock_server(floor: u64, tip: u64) -> String {
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_service_info().returning(move |_| {
        Ok(tonic::Response::new(
            sui::grpc::GetServiceInfoResponse::default()
                .with_checkpoint_height(tip)
                .with_lowest_available_checkpoint(floor),
        ))
    });
    ledger.expect_get_checkpoint().returning(move |request| {
        let checkpoint = request.into_inner().sequence_number();
        assert!(checkpoint >= floor && checkpoint <= tip);
        Ok(tonic::Response::new(
            sui::grpc::GetCheckpointResponse::default().with_checkpoint(
                sui::grpc::Checkpoint::default().with_summary(
                    sui::grpc::CheckpointSummary::default().with_timestamp(
                        std::time::UNIX_EPOCH + Duration::from_secs(checkpoint + 1),
                    ),
                ),
            ),
        ))
    });
    grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    })
}

#[tokio::test]
async fn recovery_uses_chain_time_and_includes_the_cutoff_boundary() {
    let rpc = clock_server(40, 100);
    let window = reader(&rpc).window(Duration::from_secs(30)).await.unwrap();
    assert_eq!(window.start, 69);
    assert_eq!(window.tip, 100);
    assert_eq!(window.timestamp_ms, 101_000);
    assert_eq!(
        reader(&rpc).checkpoint_at(window, 90_000).await.unwrap(),
        88
    );
}

#[tokio::test]
async fn recovery_reports_missing_history_and_accepts_a_young_chain() {
    let rpc = clock_server(80, 100);
    assert!(tokio::time::timeout(
        Duration::from_millis(250),
        reader(&rpc).window(Duration::from_secs(30)),
    )
    .await
    .expect("coverage failure must return")
    .is_err());
    let rpc = clock_server(0, 10);
    assert_eq!(
        reader(&rpc)
            .window(Duration::from_secs(48 * 60 * 60))
            .await
            .unwrap()
            .start,
        0
    );
    assert!(reader(&rpc).window(Duration::ZERO).await.is_err());
}

#[tokio::test]
async fn discovery_never_returns_a_partial_scan_as_success() {
    use sui::grpc::QueryEndReason::LedgerTip;
    for response in [
        Err(tonic::Status::out_of_range("checkpoint was pruned")),
        Ok(stream(vec![frame(10, &[2], b"item", None)])),
        Ok(stream(vec![frame(10, &[2], b"tip", Some(LedgerTip))])),
        Ok(interrupted_stream(
            vec![frame(10, &[2], b"item", None)],
            tonic::Status::unavailable("stream failed"),
        )),
    ] {
        let mut ledger = MockLedgerService::new();
        ledger
            .expect_list_transactions()
            .once()
            .return_once(move |_| response);
        ledger.expect_batch_get_objects().never();
        let rpc = grpc::mock_server(grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        assert!(tokio::time::timeout(Duration::from_secs(2), discover(&rpc))
            .await
            .expect("failure must return to caller")
            .is_err());
    }
}

#[tokio::test]
async fn dense_parallel_scans_preserve_all_objects_across_pages() {
    use sui::grpc::QueryEndReason::{CheckpointBound, ItemLimit};
    const TRANSACTIONS: u64 = 100_000;
    const OBJECTS: u64 = 10_000;
    let mut ledger = MockLedgerService::new();
    ledger.expect_list_transactions().returning(move |request| {
        let request = request.into_inner();
        let start = request.start_checkpoint.unwrap();
        let end = request.end_checkpoint.unwrap();
        let first = request
            .options
            .unwrap()
            .after
            .as_deref()
            .map_or(start, |cursor| {
                u64::from_be_bytes(cursor.try_into().unwrap()) + 1
            });
        let next = (first + 1_000).min(end);
        let frames = (first..next).map(move |checkpoint| {
            Ok(frame(
                checkpoint,
                &[checkpoint % OBJECTS],
                &checkpoint.to_be_bytes(),
                (checkpoint + 1 == next && next < end).then_some(ItemLimit),
            ))
        });
        let terminal =
            (next == end).then(|| Ok(frame(0, &[], &end.to_be_bytes(), Some(CheckpointBound))));
        Ok(tonic::Response::new(
            Box::pin(futures::stream::iter(frames.chain(terminal)))
                as grpc::BoxListTransactionsStream,
        ))
    });
    let expected = (0..OBJECTS).map(id).collect::<BTreeSet<_>>();
    let observed = metadata(&mut ledger, expected.clone());
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let started = std::time::Instant::now();
    let objects = reader(&rpc)
        .discover_work_objects(
            &sui_mocks::mock_nexus_context(),
            RecoveryWindow {
                start: 0,
                tip: TRANSACTIONS - 1,
                timestamp_ms: 0,
            },
            NonZeroUsize::new(8).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        objects.tasks.len() + objects.executions.len(),
        OBJECTS as usize
    );
    assert_eq!(*observed.lock().unwrap(), expected);
    println!(
        "Discovered {TRANSACTIONS} transactions and {OBJECTS} unique current objects in {:?}",
        started.elapsed()
    );
}

fn interrupted_stream(
    frames: Vec<sui::grpc::ListTransactionsResponse>,
    error: tonic::Status,
) -> tonic::Response<grpc::BoxListTransactionsStream> {
    // Let tonic flush the valid frames before delivering the stream error.
    let failure = futures::stream::once(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        Err(error)
    });
    tonic::Response::new(Box::pin(
        futures::stream::iter(frames.into_iter().map(Ok)).chain(failure),
    ))
}

#[tokio::test]
async fn terminal_watermark_can_repeat_the_last_item_cursor() {
    use sui::grpc::QueryEndReason::{CheckpointBound, ScanLimit};
    let mut ledger = MockLedgerService::new();
    let mut sequence = mockall::Sequence::new();
    ledger
        .expect_list_transactions()
        .once()
        .in_sequence(&mut sequence)
        .returning(|_| {
            Ok(stream(vec![
                frame(10, &[2], b"item", None),
                frame(0, &[], b"item", Some(ScanLimit)),
            ]))
        });
    ledger
        .expect_list_transactions()
        .once()
        .in_sequence(&mut sequence)
        .returning(|request| {
            assert_eq!(request.into_inner().options.unwrap().after(), b"item");
            Ok(stream(vec![frame(0, &[], b"item", Some(CheckpointBound))]))
        });
    let expected = BTreeSet::from([id(2)]);
    metadata(&mut ledger, expected.clone());
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let work = discover(&rpc).await.expect("terminal cursor may repeat");
    assert_eq!(work.tasks, expected);
}

#[tokio::test]
async fn stalled_cursor_cannot_repeat_a_page_forever() {
    use sui::grpc::QueryEndReason::ScanLimit;
    let mut ledger = MockLedgerService::new();
    ledger
        .expect_list_transactions()
        .times(2)
        .returning(|_| Ok(stream(vec![frame(0, &[], b"same", Some(ScanLimit))])));
    ledger.expect_batch_get_objects().never();
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let error = discover(&rpc).await.unwrap_err();
    assert!(error.to_string().contains("cursor"));
}

#[tokio::test]
async fn invalid_frames_commit_neither_ids_nor_cursor() {
    use sui::grpc::QueryEndReason::{CheckpointBound, CursorBound};
    let mut invalid_object = frame(11, &[4, 6], b"bad", None);
    invalid_object
        .transaction
        .as_mut()
        .unwrap()
        .effects
        .as_mut()
        .unwrap()
        .changed_objects[1]
        .object_id = None;
    let mut missing_cursor = frame(11, &[4], b"bad", None);
    missing_cursor.watermark = None;
    let mut early_completion = frame(0, &[], b"bad", Some(CheckpointBound));
    early_completion.watermark.as_mut().unwrap().checkpoint = Some(19);
    let cases = [
        early_completion,
        invalid_object,
        missing_cursor,
        frame(99, &[4], b"bad", None),
        frame(11, &[4], b"bad", Some(CursorBound)),
    ];
    for invalid in cases {
        let mut ledger = MockLedgerService::new();
        ledger
            .expect_list_transactions()
            .once()
            .return_once(move |_| Ok(stream(vec![frame(10, &[2], b"good", None), invalid])));
        ledger.expect_batch_get_objects().never();
        let rpc = grpc::mock_server(grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        assert!(discover(&rpc).await.is_err());
    }
}

#[tokio::test]
async fn failed_metadata_is_not_a_partial_success() {
    let mut ledger = MockLedgerService::new();
    ledger.expect_list_transactions().once().returning(|_| {
        Ok(stream(vec![
            frame(10, &[2], b"item", None),
            frame(
                0,
                &[],
                b"end",
                Some(sui::grpc::QueryEndReason::CheckpointBound),
            ),
        ]))
    });
    ledger
        .expect_batch_get_objects()
        .once()
        .returning(|_| Err(tonic::Status::unavailable("metadata unavailable")));
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    assert!(discover(&rpc).await.is_err());
}

#[tokio::test(start_paused = true)]
async fn failures_and_timeouts_are_single_observations() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let reader = reader("http://localhost:1");
    let calls = AtomicUsize::new(0);
    let result: anyhow::Result<()> = reader
        .read("unavailable read", || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { anyhow::bail!("endpoint is unavailable") }
        })
        .await;
    assert!(format!("{:#}", result.unwrap_err()).contains("endpoint is unavailable"));
    let started = tokio::time::Instant::now();
    let result = reader
        .read("hanging read", || async {
            calls.fetch_add(1, Ordering::SeqCst);
            futures::future::pending::<anyhow::Result<()>>().await
        })
        .await;
    assert!(format!("{:#}", result.unwrap_err()).contains("timed out"));
    assert_eq!(started.elapsed(), Duration::from_secs(30));
    tokio::time::sleep(Duration::from_secs(60)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn cancellation_releases_an_active_read() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reading<'a>(&'a AtomicUsize);
    impl Drop for Reading<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let reader = reader("http://localhost:1");
    let active = AtomicUsize::new(0);
    let read = reader.read("stalled read", || async {
        active.fetch_add(1, Ordering::SeqCst);
        let _reading = Reading(&active);
        futures::future::pending::<anyhow::Result<()>>().await
    });
    assert!(tokio::time::timeout(Duration::from_secs(1), read)
        .await
        .is_err());
    assert_eq!(active.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn slow_healthy_reads_finish_without_restarting() {
    let reader = reader("http://localhost:1");
    let attempts = std::sync::atomic::AtomicUsize::new(0);
    let completed = tokio::time::timeout(
        Duration::from_secs(60),
        reader.read("healthy sequence", || async {
            attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            for _ in 0..8 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(8)
        }),
    )
    .await
    .expect("healthy progress did not finish");
    assert_eq!(completed.unwrap(), 8);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn checkpoint_search_reports_unavailable_evidence() {
    let mut ledger = MockLedgerService::new();
    ledger
        .expect_get_checkpoint()
        .once()
        .returning(|_| Err(tonic::Status::deadline_exceeded("checkpoint unavailable")));
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    assert!(reader(&rpc)
        .checkpoint_at(
            RecoveryWindow {
                start: 40,
                tip: 100,
                timestamp_ms: 101_000,
            },
            90_000
        )
        .await
        .is_err());
}

#[tokio::test]
async fn recovery_reports_unavailable_or_malformed_coverage() {
    for response in [
        Err(tonic::Status::unavailable("service unavailable")),
        Ok(sui::grpc::GetServiceInfoResponse::default()),
    ] {
        let mut ledger = MockLedgerService::new();
        ledger
            .expect_get_service_info()
            .once()
            .return_once(move |_| response.map(tonic::Response::new));
        ledger.expect_get_checkpoint().never();
        let rpc = grpc::mock_server(grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        assert!(reader(&rpc).window(Duration::from_secs(30)).await.is_err());
    }
}

#[test]
fn invalid_endpoint_is_rejected_before_reading() {
    assert!(RecoveryReader::new(":invalid endpoint").is_err());
}

#[tokio::test]
async fn cancelling_an_open_scan_releases_the_server_stream() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reading(Arc<AtomicUsize>);
    impl Drop for Reading {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let active = Arc::new(AtomicUsize::new(0));
    let observed = active.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let mut ledger = MockLedgerService::new();
    ledger
        .expect_list_transactions()
        .once()
        .return_once(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            let reading = Reading(observed);
            let stalled = futures::stream::once(async move {
                let _reading = reading;
                let _ = started.send(());
                futures::future::pending().await
            });
            Ok(tonic::Response::new(Box::pin(
                futures::stream::iter([Ok(frame(10, &[2], b"item", None))]).chain(stalled),
            )))
        });
    ledger.expect_batch_get_objects().never();
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let reader = reader(&rpc);
    let context = sui_mocks::mock_nexus_context();
    let mut discovery = Box::pin(reader.discover_work_objects(
        &context,
        RecoveryWindow {
            start: 10,
            tip: 20,
            timestamp_ms: 0,
        },
        NonZeroUsize::MIN,
    ));
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            result = ready => result.unwrap(),
            _ = &mut discovery => panic!("an incomplete scan returned"),
        }
    })
    .await
    .expect("scan did not open");
    drop(discovery);
    tokio::time::timeout(Duration::from_secs(2), async {
        while active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("cancelled scan retained its server stream");
}

#[test]
fn recovery_futures_can_run_on_worker_tasks() {
    fn assert_send(_: impl Send) {}
    let reader = reader("http://localhost:1");
    let window = RecoveryWindow {
        start: 10,
        tip: 20,
        timestamp_ms: 0,
    };
    let context = sui_mocks::mock_nexus_context();
    assert_send(reader.window(Duration::from_secs(30)));
    assert_send(reader.checkpoint_at(window, 0));
    assert_send(reader.discover_work_objects(&context, window, NonZeroUsize::MIN));
    assert_send(reader.read("application read", || async { Ok(()) }));
}
