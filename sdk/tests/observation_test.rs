#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        nexus::crawler::Crawler,
        sui::{
            self,
            observation::{self, Observer, Operation, Outcome, Traffic},
        },
        test_utils::sui_mocks::grpc::{mock_server, MockLedgerService, ServerMocks},
    },
    std::{
        num::NonZeroU32,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
            Mutex,
        },
        time::Duration,
    },
};

#[derive(Default)]
struct Records {
    started: Vec<Operation>,
    finished: Vec<(Operation, Outcome)>,
}
struct Recorder(Arc<Mutex<Records>>);
impl Observer for Recorder {
    fn started(&self, operation: Operation) {
        self.0.lock().unwrap().started.push(operation);
    }

    fn finished(&self, operation: Operation, _: Duration, outcome: Outcome) {
        self.0.lock().unwrap().finished.push((operation, outcome));
    }
}

// This binary installs one process observer and exercises the real client layer,
// retry scope, concurrent preparation, and cancellation before request dispatch.
#[tokio::test]
async fn observations_account_for_retries_without_replaying_healthy_reads() {
    let records = Arc::new(Mutex::new(Records::default()));
    assert!(observation::install(Box::new(Recorder(Arc::clone(&records)))).is_ok());
    let healthy = sui::types::Address::from_static("0x1");
    let failing = sui::types::Address::from_static("0x2");
    let failed_attempts = Arc::new(AtomicUsize::new(0));
    let mut ledger = MockLedgerService::new();
    ledger
        .expect_get_object()
        .times(3)
        .returning(move |request| {
            let id = request
                .get_ref()
                .object_id()
                .parse::<sui::types::Address>()
                .unwrap();
            if id == failing && failed_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(tonic::Status::unavailable("injected interruption"));
            }
            let mut object = sui::grpc::Object::default();
            object.set_object_id(id);
            object.set_object_type("0x2::clock::Clock");
            object.set_version(1);
            object.set_digest(sui::types::Digest::ZERO);
            object.set_previous_transaction(sui::types::Digest::ZERO);
            object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Immutable));
            Ok(tonic::Response::new(
                sui::grpc::GetObjectResponse::default().with_object(object),
            ))
        });
    let url = mock_server(ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let crawler = Crawler::new(Arc::new(sui::grpc::client(&url).unwrap()));
    sui::grpc::with_read_retry_until(
        tokio::time::Instant::now() + Duration::from_secs(3),
        async {
            tokio::try_join!(
                crawler.get_object_update_reference(healthy, None),
                crawler.get_object_update_reference(failing, None)
            )
        },
    )
    .await
    .unwrap();
    {
        let records = records.lock().unwrap();
        assert_eq!(
            records
                .started
                .iter()
                .filter(|operation| **operation == Operation::Read)
                .count(),
            2
        );
        assert_eq!(
            records
                .started
                .iter()
                .filter(|operation| **operation == Operation::ReadAttempt)
                .count(),
            3
        );
        assert_eq!(
            records
                .started
                .iter()
                .filter(|operation| matches!(
                    operation,
                    Operation::Rpc {
                        traffic: Traffic::Normal,
                        ..
                    }
                ))
                .count(),
            2
        );
        assert_eq!(
            records
                .started
                .iter()
                .filter(|operation| matches!(
                    operation,
                    Operation::Rpc {
                        traffic: Traffic::Recovery,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert_eq!(
            records
                .finished
                .iter()
                .filter(
                    |(operation, outcome)| matches!(operation, Operation::Rpc { .. })
                        && *outcome == Outcome::Error(Some(tonic::Code::Unavailable))
                )
                .count(),
            1
        );
    }

    let one = NonZeroU32::new(1).unwrap();
    sui::grpc::set_retry_request_budget(&url, one, one).unwrap();
    let mut client = sui::grpc::client(&url).unwrap();
    sui::grpc::with_retry_budget(
        client
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default()),
    )
    .await
    .unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(5),
        sui::grpc::with_retry_budget(
            client
                .ledger_client()
                .get_service_info(sui::grpc::GetServiceInfoRequest::default())
        )
    )
    .await
    .is_err());
    let records = records.lock().unwrap();
    assert_eq!(
        records
            .started
            .iter()
            .filter(|operation| matches!(operation, Operation::Rpc { .. }))
            .count(),
        4
    );
    assert!(records
        .finished
        .contains(&(Operation::RequestAdmission, Outcome::Cancelled)));
    assert_eq!(records.started.len(), records.finished.len());
}
