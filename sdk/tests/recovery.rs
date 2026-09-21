#![cfg(feature = "test_utils")]

use {
    futures::StreamExt as _,
    nexus_sdk::{
        move_bindings::{self, scheduler::task::Task, workflow::execution::DAGExecution},
        nexus::recovery::{ListConfig, ListEvent, RecoveryReader, RecoveryWindow},
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

fn policy() -> ListConfig {
    let mut policy = ListConfig::default();
    policy.base_retry_delay = Duration::from_millis(1);
    policy.max_retry_delay = Duration::from_millis(5);
    policy.retry_jitter = Duration::ZERO;
    policy
}

fn reader(rpc: &str) -> RecoveryReader<'_> {
    RecoveryReader::new(rpc, policy()).unwrap()
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
    use sui::grpc::QueryEndReason::{CheckpointBound, ItemLimit, LedgerTip, ScanLimit};
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
            vec![frame(0, &[], b"tip", Some(LedgerTip))],
        ),
        (
            Some(b"tip".as_slice()),
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
async fn recovery_waits_for_complete_history_and_accepts_a_young_chain() {
    let rpc = clock_server(80, 100);
    assert!(tokio::time::timeout(
        Duration::from_millis(250),
        reader(&rpc).window(Duration::from_secs(30)),
    )
    .await
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
    for pruned in [false, true] {
        let mut ledger = MockLedgerService::new();
        ledger
            .expect_list_transactions()
            .times(2..)
            .returning(move |_| {
                if pruned {
                    return Err(tonic::Status::out_of_range("checkpoint was pruned"));
                }
                Ok(stream(vec![frame(10, &[2], b"item", None)]))
            });
        ledger.expect_batch_get_objects().never();
        let rpc = grpc::mock_server(grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        assert!(tokio::time::timeout(
            Duration::from_millis(250),
            reader(&rpc).discover_work_objects(
                &sui_mocks::mock_nexus_context(),
                RecoveryWindow {
                    start: 10,
                    tip: 20,
                    timestamp_ms: 0,
                },
                NonZeroUsize::MIN,
            ),
        )
        .await
        .is_err());
    }
}

#[tokio::test]
async fn dense_parallel_scans_preserve_all_objects_through_interruptions() {
    use sui::grpc::QueryEndReason::{CheckpointBound, ItemLimit};
    const TRANSACTIONS: u64 = 100_000;
    const OBJECTS: u64 = 10_000;
    let mut ledger = MockLedgerService::new();
    let interrupted = Arc::new(Mutex::new(BTreeSet::new()));
    let interrupted_scans = Arc::clone(&interrupted);
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
        if interrupted_scans.lock().unwrap().insert(start) {
            return Ok(interrupted_stream(
                (first..(first + 500).min(end))
                    .map(|checkpoint| {
                        frame(
                            checkpoint,
                            &[checkpoint % OBJECTS],
                            &checkpoint.to_be_bytes(),
                            None,
                        )
                    })
                    .collect(),
                tonic::Status::deadline_exceeded("scan interrupted"),
            ));
        }
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
    assert_eq!(interrupted.lock().unwrap().len(), 8);
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
async fn discovery_resumes_after_request_and_stream_failures() {
    use sui::grpc::QueryEndReason::CheckpointBound;
    let mut ledger = MockLedgerService::new();
    let mut sequence = mockall::Sequence::new();
    let cases = [
        (
            None,
            Err(tonic::Status::deadline_exceeded("request timed out")),
        ),
        (
            None,
            Ok(interrupted_stream(
                vec![
                    frame(10, &[2, 3], b"item", None),
                    frame(0, &[], b"scanned", None),
                ],
                tonic::Status::deadline_exceeded("stream timed out"),
            )),
        ),
        (
            Some(b"scanned".as_slice()),
            Err(tonic::Status::resource_exhausted("node is busy")),
        ),
        (
            Some(b"scanned".as_slice()),
            Ok(stream(vec![
                frame(20, &[3, 4], b"last", None),
                frame(0, &[], b"end", Some(CheckpointBound)),
            ])),
        ),
    ];
    for (after, response) in cases {
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
                response
            });
    }
    let observed = metadata(&mut ledger, [id(2), id(3), id(4)].into());
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    use std::sync::atomic::{AtomicUsize, Ordering};
    let retries = Arc::new(AtomicUsize::new(0));
    let events = retries.clone();
    let policy = policy().with_observer(move |event| {
        if matches!(event, ListEvent::RetryScheduled { .. }) {
            events.fetch_add(1, Ordering::SeqCst);
        }
    });
    let reader = RecoveryReader::new(&rpc, policy).unwrap();
    let objects = tokio::time::timeout(
        Duration::from_secs(5),
        reader.discover_work_objects(
            &sui_mocks::mock_nexus_context(),
            RecoveryWindow {
                start: 10,
                tip: 20,
                timestamp_ms: 0,
            },
            NonZeroUsize::MIN,
        ),
    )
    .await
    .expect("discovery did not resume")
    .unwrap();
    assert_eq!(retries.load(Ordering::SeqCst), 3);
    assert_eq!(objects.tasks, [id(2), id(4)].into());
    assert_eq!(objects.executions, [id(3)].into());
    assert_eq!(*observed.lock().unwrap(), [id(2), id(3), id(4)].into());
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
        let mut sequence = mockall::Sequence::new();
        ledger
            .expect_list_transactions()
            .once()
            .in_sequence(&mut sequence)
            .return_once(move |request| {
                assert!(request.into_inner().options.unwrap().after.is_none());
                Ok(stream(vec![frame(10, &[2], b"good", None), invalid]))
            });
        ledger
            .expect_list_transactions()
            .once()
            .in_sequence(&mut sequence)
            .return_once(|request| {
                assert_eq!(
                    request.into_inner().options.unwrap().after.as_deref(),
                    Some(b"good".as_slice())
                );
                Ok(stream(vec![
                    frame(20, &[3], b"last", None),
                    frame(0, &[], b"end", Some(CheckpointBound)),
                ]))
            });
        let observed = metadata(&mut ledger, [id(2), id(3)].into());
        let rpc = grpc::mock_server(grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        let objects = tokio::time::timeout(
            Duration::from_secs(2),
            reader(&rpc).discover_work_objects(
                &sui_mocks::mock_nexus_context(),
                RecoveryWindow {
                    start: 10,
                    tip: 20,
                    timestamp_ms: 0,
                },
                NonZeroUsize::MIN,
            ),
        )
        .await
        .expect("discovery did not retry invalid frame")
        .unwrap();
        assert_eq!(objects.tasks, [id(2)].into());
        assert_eq!(objects.executions, [id(3)].into());
        assert_eq!(*observed.lock().unwrap(), [id(2), id(3)].into());
    }
}

#[tokio::test]
async fn discovery_retries_metadata_without_repeating_the_scan() {
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
    let mut sequence = mockall::Sequence::new();
    ledger
        .expect_batch_get_objects()
        .once()
        .in_sequence(&mut sequence)
        .returning(|_| Err(tonic::Status::unavailable("metadata unavailable")));
    ledger
        .expect_batch_get_objects()
        .once()
        .in_sequence(&mut sequence)
        .returning(|_| {
            Ok(tonic::Response::new(
                sui::grpc::BatchGetObjectsResponse::default(),
            ))
        });
    let observed = metadata(&mut ledger, [id(2)].into());
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let objects = tokio::time::timeout(
        Duration::from_secs(2),
        reader(&rpc).discover_work_objects(
            &sui_mocks::mock_nexus_context(),
            RecoveryWindow {
                start: 10,
                tip: 20,
                timestamp_ms: 0,
            },
            NonZeroUsize::MIN,
        ),
    )
    .await
    .expect("metadata did not recover")
    .unwrap();
    assert_eq!(objects.tasks, [id(2)].into());
    assert_eq!(*observed.lock().unwrap(), [id(2)].into());
}

#[tokio::test(start_paused = true)]
async fn unavailable_reads_obey_policy_and_remain_cancellable() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut policy = ListConfig::default();
    policy.base_retry_delay = Duration::from_secs(1);
    policy.max_retry_delay = Duration::from_secs(4);
    policy.retry_jitter = Duration::ZERO;
    let reader = RecoveryReader::new("http://localhost:1", policy).unwrap();
    let calls = AtomicUsize::new(0);
    let result = tokio::time::timeout(
        Duration::from_millis(10_500),
        reader.read("unavailable read", || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { anyhow::bail!("endpoint is unavailable") }
        }),
    )
    .await;
    let result: Result<(), _> = result;
    assert!(result.is_err());
    // Attempts at 0, 1, 3, and 7 seconds; the next wait is capped at 4 seconds.
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    tokio::time::sleep(Duration::from_secs(60)).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "cancelled recovery kept retrying"
    );
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
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Ok(8)
        }),
    )
    .await
    .expect("healthy progress did not finish");
    assert_eq!(completed, 8);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn checkpoint_search_retains_progress_across_failed_reads() {
    use std::collections::BTreeMap;
    let counts = Arc::new(Mutex::new(BTreeMap::new()));
    let observed = counts.clone();
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_checkpoint().returning(move |request| {
        let checkpoint = request.into_inner().sequence_number();
        let mut counts = observed.lock().unwrap();
        let count = counts.entry(checkpoint).or_insert(0);
        *count += 1;
        if checkpoint == 85 {
            if *count == 1 {
                return Err(tonic::Status::deadline_exceeded("checkpoint unavailable"));
            }
            if *count == 2 {
                return Ok(tonic::Response::new(
                    sui::grpc::GetCheckpointResponse::default(),
                ));
            }
        }
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
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let checkpoint = reader(&rpc)
        .checkpoint_at(
            RecoveryWindow {
                start: 40,
                tip: 100,
                timestamp_ms: 101_000,
            },
            90_000,
        )
        .await
        .unwrap();
    assert_eq!(checkpoint, 88);
    let counts = counts.lock().unwrap();
    assert_eq!(counts[&85], 3);
    assert!(counts
        .iter()
        .filter(|(checkpoint, _)| **checkpoint != 85)
        .all(|(_, count)| *count == 1));
}

#[tokio::test]
async fn recovery_refreshes_the_window_until_retained_coverage_is_complete() {
    let mut ledger = MockLedgerService::new();
    let mut sequence = mockall::Sequence::new();
    for response in [
        Err(tonic::Status::unavailable("service unavailable")),
        Ok(sui::grpc::GetServiceInfoResponse::default()),
        Ok(sui::grpc::GetServiceInfoResponse::default()
            .with_checkpoint_height(100)
            .with_lowest_available_checkpoint(80)),
        Ok(sui::grpc::GetServiceInfoResponse::default()
            .with_checkpoint_height(100)
            .with_lowest_available_checkpoint(40)),
    ] {
        ledger
            .expect_get_service_info()
            .once()
            .in_sequence(&mut sequence)
            .return_once(move |_| response.map(tonic::Response::new));
    }
    ledger.expect_get_checkpoint().returning(|request| {
        let checkpoint = request.into_inner().sequence_number();
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
    let rpc = grpc::mock_server(grpc::ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let window = tokio::time::timeout(
        Duration::from_secs(3),
        reader(&rpc).window(Duration::from_secs(30)),
    )
    .await
    .expect("coverage did not recover")
    .unwrap();
    assert_eq!(
        (window.start, window.tip, window.timestamp_ms),
        (69, 100, 101_000)
    );
}

#[test]
fn invalid_retry_policy_is_rejected_before_reading() {
    let mut invalid = policy();
    invalid.base_retry_delay = Duration::ZERO;
    assert!(RecoveryReader::new("http://localhost:1", invalid).is_err());
    let mut invalid = policy();
    invalid.max_retry_delay = Duration::ZERO;
    assert!(RecoveryReader::new("http://localhost:1", invalid).is_err());
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
