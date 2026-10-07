#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        nexus::crawler::Crawler,
        sui::{
            self,
            observation::{self, Observer, Operation, Outcome},
        },
        test_utils::sui_mocks::grpc::MockLedgerService,
    },
    std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    },
};

#[derive(Default)]
struct Counts {
    active: AtomicUsize,
    peak: AtomicUsize,
    finished: AtomicUsize,
    failed: AtomicUsize,
}

struct Counter(Arc<Counts>);
impl Observer for Counter {
    fn started(&self, operation: Operation) {
        if matches!(operation, Operation::Rpc { .. }) {
            let active = self.0.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.0.peak.fetch_max(active, Ordering::SeqCst);
        }
    }

    fn finished(&self, operation: Operation, _: Duration, outcome: Outcome) {
        if matches!(operation, Operation::Rpc { .. }) {
            self.0.active.fetch_sub(1, Ordering::SeqCst);
            self.0.finished.fetch_add(1, Ordering::SeqCst);
            if outcome != Outcome::Success {
                self.0.failed.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

#[tokio::test]
async fn observed_requests_share_a_connection_without_serializing_responses() {
    const REQUESTS: usize = 32;
    let counts = Arc::new(Counts::default());
    assert!(observation::install(Box::new(Counter(Arc::clone(&counts)))).is_ok());
    let gate = Arc::new(tokio::sync::Barrier::new(REQUESTS + 1));
    let server_gate = Arc::clone(&gate);
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_object().times(REQUESTS).returning(|_| {
        let mut object = sui::grpc::Object::default();
        object.set_object_id(sui::types::Address::ZERO);
        object.set_object_type("0x2::clock::Clock");
        object.set_version(1);
        object.set_digest(sui::types::Digest::ZERO);
        object.set_previous_transaction(sui::types::Digest::ZERO);
        object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Immutable));
        Ok(tonic::Response::new(
            sui::grpc::GetObjectResponse::default().with_object(object),
        ))
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(tower::ServiceBuilder::new().map_future(move |future| {
                let gate = Arc::clone(&server_gate);
                async move {
                    // No response can complete until every request reaches the server.
                    gate.wait().await;
                    future.await
                }
            }))
            .add_service(sui::grpc::ledger_service_server::LedgerServiceServer::new(
                ledger,
            ))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let crawler = Crawler::new(Arc::new(sui::grpc::client(&url).unwrap()));
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..REQUESTS {
        let crawler = crawler.clone();
        requests.spawn(async move {
            crawler
                .get_object_update_reference(sui::types::Address::ZERO, None)
                .await
        });
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        gate.wait().await;
        while let Some(result) = requests.join_next().await {
            result.unwrap().unwrap();
        }
    })
    .await
    .expect("all requests must reach the server before any response completes");
    assert_eq!(counts.peak.load(Ordering::SeqCst), REQUESTS);
    assert_eq!(counts.finished.load(Ordering::SeqCst), REQUESTS);
    assert_eq!(counts.active.load(Ordering::SeqCst), 0);
    assert_eq!(counts.failed.load(Ordering::SeqCst), 0);
    server.abort();
    let _ = server.await;
}
