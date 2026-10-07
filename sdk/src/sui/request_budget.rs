//! Shared admission for recovery RPC attempts to a configured endpoint.

use {
    super::observation::{Observation, Operation, Outcome},
    std::{
        num::NonZeroU32,
        sync::{Arc, Mutex},
        time::Duration,
    },
    tokio::time::Instant,
};

tokio::task_local! { static RETRY: (); }
tokio::task_local! { static READ_DEADLINE: Option<Instant>; }

pub(super) fn is_retry() -> bool {
    RETRY.try_with(|_| ()).is_ok()
}

pub(super) async fn scope<F: std::future::Future>(attempt: F) -> F::Output {
    RETRY.scope((), attempt).await
}

pub(super) async fn read_scope<F: std::future::Future>(
    deadline: Option<Instant>,
    attempt: F,
) -> F::Output {
    READ_DEADLINE.scope(deadline, attempt).await
}

/// Retain the surrounding preparation while only its failed observation waits.
pub(super) async fn read<T, F, Fut>(operation: F) -> Result<T, tonic::Status>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, tonic::Status>>,
{
    let observation = Observation::start(Operation::Read);
    let result = read_inner(operation).await;
    observation.finish(read_outcome(&result));
    result
}

fn read_outcome<T>(result: &Result<T, tonic::Status>) -> Outcome {
    match result {
        Ok(_) => Outcome::Success,
        Err(status) if status.code() == tonic::Code::DeadlineExceeded => Outcome::Deadline,
        Err(status) if status.code() == tonic::Code::Cancelled => Outcome::Cancelled,
        Err(status) => Outcome::Error(Some(status.code())),
    }
}

async fn attempt<T>(
    future: impl std::future::Future<Output = Result<T, tonic::Status>>,
) -> Result<T, tonic::Status> {
    let observation = Observation::start(Operation::ReadAttempt);
    let result = future.await;
    observation.finish(read_outcome(&result));
    result
}

async fn read_inner<T, F, Fut>(mut operation: F) -> Result<T, tonic::Status>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, tonic::Status>>,
{
    let Some(deadline) = READ_DEADLINE.try_with(|deadline| *deadline).ok().flatten() else {
        return attempt(operation()).await;
    };
    let mut delay = Duration::from_millis(250);
    let mut retrying = false;
    loop {
        let attempt = async {
            if retrying {
                scope(attempt(operation())).await
            } else {
                attempt(operation()).await
            }
        };
        let result = tokio::time::timeout_at(deadline, attempt)
            .await
            .map_err(|_| tonic::Status::deadline_exceeded("Observation window ended"))?;
        match result {
            Ok(value) => return Ok(value),
            Err(status)
                if matches!(
                    status.code(),
                    tonic::Code::Unavailable
                        | tonic::Code::ResourceExhausted
                        | tonic::Code::DeadlineExceeded
                        | tonic::Code::Internal
                        | tonic::Code::Unknown
                ) =>
            {
                let wait = delay.mul_f64(0.5 + rand::random::<f64>() * 0.5);
                let observation = Observation::start(Operation::RetryDelay);
                let waited = tokio::time::timeout_at(deadline, tokio::time::sleep(wait)).await;
                observation.finish(if waited.is_ok() {
                    Outcome::Success
                } else {
                    Outcome::Deadline
                });
                waited.map_err(|_| tonic::Status::deadline_exceeded("Observation window ended"))?;
                delay = delay.saturating_mul(2).min(Duration::from_secs(5));
                retrying = true;
            }
            Err(status) => return Err(status),
        }
    }
}

/// Connection pools bound sockets; this bucket separately bounds request volume
/// across those sockets without retrying or retaining any request body.
#[derive(Default)]
pub(super) struct RequestBudget {
    rate: Option<NonZeroU32>,
    burst: u32,
    tokens: f64,
    refreshed: Option<Instant>,
}

impl RequestBudget {
    pub(super) fn configure(&mut self, rate: NonZeroU32, burst: NonZeroU32) {
        if self.rate == Some(rate) && self.burst == burst.get() {
            return;
        }
        self.rate = Some(rate);
        self.burst = burst.get();
        self.tokens = f64::from(burst.get());
        self.refreshed = Some(Instant::now());
    }

    fn delay(&mut self, now: Instant) -> Option<Duration> {
        let rate = f64::from(self.rate?.get());
        let elapsed = now.saturating_duration_since(self.refreshed.unwrap_or(now));
        self.refreshed = Some(now);
        self.tokens = (self.tokens + elapsed.as_secs_f64() * rate).min(f64::from(self.burst));
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            None
        } else {
            Some(Duration::from_secs_f64((1.0 - self.tokens) / rate))
        }
    }
}

pub(super) async fn acquire(budget: Arc<Mutex<RequestBudget>>) {
    let observation = Observation::start(Operation::RequestAdmission);
    loop {
        let delay = budget
            .lock()
            .expect("RPC budget lock poisoned")
            .delay(Instant::now());
        match delay {
            None => {
                observation.finish(Outcome::Success);
                return;
            }
            Some(delay) => tokio::time::sleep(delay).await,
        }
    }
}

/// The bucket has no transport readiness state. This wrapper admits a retry
/// before the underlying service can dispatch it, without cloning its body.
pub(super) struct BudgetService<S> {
    inner: S,
    budget: Arc<Mutex<RequestBudget>>,
    waiting: Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
    admitted: bool,
}

impl<S> BudgetService<S> {
    pub(super) fn new(inner: S, budget: Arc<Mutex<RequestBudget>>) -> Self {
        Self {
            inner,
            budget,
            waiting: None,
            admitted: false,
        }
    }
}

impl<S, Request> tower::Service<Request> for BudgetService<S>
where
    S: tower::Service<Request>,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if !self.admitted && is_retry() {
            let waiting = self
                .waiting
                .get_or_insert_with(|| Box::pin(acquire(Arc::clone(&self.budget))));
            std::task::ready!(waiting.as_mut().poll(cx));
            self.waiting = None;
            self.admitted = true;
        }
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        self.admitted = false;
        self.waiting = None;
        self.inner.call(request)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::atomic::{AtomicUsize, Ordering},
        tower::ServiceExt as _,
    };

    #[tokio::test(start_paused = true)]
    async fn retrying_one_read_preserves_completed_parallel_preparation() {
        let healthy = AtomicUsize::new(0);
        let failing = AtomicUsize::new(0);
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = read_scope(Some(deadline), async {
            tokio::try_join!(
                read(|| async {
                    healthy.fetch_add(1, Ordering::SeqCst);
                    Ok(7)
                }),
                read(|| async {
                    let attempt = failing.fetch_add(1, Ordering::SeqCst);
                    if attempt < 2 {
                        Err(tonic::Status::unavailable("temporarily unavailable"))
                    } else {
                        assert!(is_retry());
                        Ok(9)
                    }
                }),
            )
        })
        .await
        .unwrap();
        assert_eq!(result, (7, 9));
        assert_eq!(healthy.load(Ordering::SeqCst), 1);
        assert_eq!(failing.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn read_deadline_cancels_backoff_and_permanent_errors_are_not_retried() {
        let calls = AtomicUsize::new(0);
        let started = Instant::now();
        let error = read_scope(
            Some(started + Duration::from_millis(10)),
            read(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(tonic::Status::unavailable("offline"))
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), tonic::Code::DeadlineExceeded);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(started.elapsed(), Duration::from_millis(10));

        let error = read_scope(
            Some(Instant::now() + Duration::from_secs(5)),
            read(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(tonic::Status::permission_denied("denied"))
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), tonic::Code::PermissionDenied);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn healthy_calls_bypass_exhausted_recovery_admission() {
        let budget = Arc::new(Mutex::new(RequestBudget::default()));
        let one = NonZeroU32::new(1).unwrap();
        budget.lock().unwrap().configure(one, one);
        acquire(Arc::clone(&budget)).await;
        let make_service = || {
            BudgetService::new(
                tower::service_fn(|()| async { Ok::<_, tonic::Status>(()) }),
                Arc::clone(&budget),
            )
        };
        let started = Instant::now();
        let recovery = tokio::spawn(scope(make_service().oneshot(())));
        let mut jobs = tokio::task::JoinSet::new();
        for _ in 0..100 {
            jobs.spawn(make_service().oneshot(()));
        }
        while let Some(result) = jobs.join_next().await {
            result.unwrap().unwrap();
        }
        assert_eq!(started.elapsed(), Duration::ZERO);
        recovery.await.unwrap().unwrap();
        assert!(started.elapsed() >= Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_attempts_share_the_burst_and_refill() {
        let budget = Arc::new(Mutex::new(RequestBudget::default()));
        budget
            .lock()
            .unwrap()
            .configure(NonZeroU32::new(10).unwrap(), NonZeroU32::new(2).unwrap());
        let started = Instant::now();
        let mut jobs = tokio::task::JoinSet::new();
        for _ in 0..12 {
            let budget = Arc::clone(&budget);
            jobs.spawn(async move {
                acquire(budget).await;
                Instant::now()
            });
        }
        let mut finished = Vec::new();
        while let Some(result) = jobs.join_next().await {
            finished.push(result.unwrap());
        }
        finished.sort();
        assert!(finished[2] - started >= Duration::from_millis(100));
        assert!(finished[11] - started >= Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_and_reconfiguration_do_not_replenish_the_budget() {
        let budget = Arc::new(Mutex::new(RequestBudget::default()));
        let one = NonZeroU32::new(1).unwrap();
        budget.lock().unwrap().configure(one, one);
        acquire(Arc::clone(&budget)).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(100), acquire(Arc::clone(&budget)))
                .await
                .is_err()
        );
        budget.lock().unwrap().configure(one, one);
        let started = Instant::now();
        acquire(budget).await;
        assert!(started.elapsed() >= Duration::from_millis(900));
    }
}
