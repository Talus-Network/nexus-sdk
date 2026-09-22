//! Observe actual requests after admission, through the final gRPC trailers.
//! HTTP success alone does not establish RPC success. Streams remain active
//! until they finish or their consumer drops them.

use {
    super::observation::{Observation, Operation, Outcome, RpcMethod, Traffic},
    http::{Request, Response},
    http_body::Body as _,
    std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    },
    tonic::body::Body,
};

pub(super) struct ObservedService<S>(pub(super) S);

impl<S> tower::Service<Request<Body>> for ObservedService<S>
where
    S: tower::Service<Request<Body>, Response = Response<Body>>,
{
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;
    type Response = Response<Body>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let method = request.uri().path().rsplit('/').next().unwrap_or_default();
        let guard = Observation::start(Operation::Rpc {
            method: RpcMethod::from_path(method),
            traffic: if super::request_budget::is_retry() {
                Traffic::Recovery
            } else {
                Traffic::Normal
            },
        });
        ResponseFuture {
            inner: self.0.call(request),
            guard: guard.is_active().then_some(guard),
        }
    }
}

pin_project_lite::pin_project! {
    pub(super) struct ResponseFuture<F> {
        #[pin]
        inner: F,
        guard: Option<Observation>,
    }
}

impl<F, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<Body>, E>>,
{
    type Output = Result<Response<Body>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match std::task::ready!(this.inner.poll(cx)) {
            Ok(response) => {
                let (parts, body) = response.into_parts();
                let mut guard = this.guard.take();
                if let Some(outcome) = status(&parts.headers) {
                    finish(&mut guard, outcome);
                } else if !parts.status.is_success() || body.is_end_stream() {
                    finish(&mut guard, Outcome::Error(None));
                }
                // Preserve the original body when observation is absent or complete.
                let body = if guard.is_some() {
                    Body::new(ObservedBody { inner: body, guard })
                } else {
                    body
                };
                Poll::Ready(Ok(Response::from_parts(parts, body)))
            }
            Err(error) => {
                finish(this.guard, Outcome::Error(None));
                Poll::Ready(Err(error))
            }
        }
    }
}

fn status(headers: &http::HeaderMap) -> Option<Outcome> {
    let value = headers.get("grpc-status")?.to_str().ok();
    Some(match value {
        Some("0") => Outcome::Success,
        Some("1") => Outcome::Cancelled,
        Some("4") => Outcome::Deadline,
        _ => Outcome::Error(Some(tonic::Code::from_bytes(
            value.unwrap_or("2").as_bytes(),
        ))),
    })
}

fn finish(guard: &mut Option<Observation>, outcome: Outcome) {
    if let Some(guard) = guard.take() {
        guard.finish(outcome);
    }
}

pub(super) struct ObservedBody {
    inner: Body,
    guard: Option<Observation>,
}

impl http_body::Body for ObservedBody {
    type Data = <Body as http_body::Body>::Data;
    type Error = <Body as http_body::Body>::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let frame = std::task::ready!(Pin::new(&mut self.inner).poll_frame(cx));
        match &frame {
            Some(Ok(frame)) => {
                if let Some(outcome) = frame.trailers_ref().and_then(status) {
                    finish(&mut self.guard, outcome);
                }
            }
            Some(Err(error)) => finish(
                &mut self.guard,
                match error.code() {
                    tonic::Code::DeadlineExceeded => Outcome::Deadline,
                    tonic::Code::Cancelled => Outcome::Cancelled,
                    code => Outcome::Error(Some(code)),
                },
            ),
            None => finish(&mut self.guard, Outcome::Error(None)),
        }
        Poll::Ready(frame)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grpc_status_controls_outcome() {
        let mut headers = http::HeaderMap::new();
        assert_eq!(status(&headers), None);
        for (code, outcome) in [
            ("0", Outcome::Success),
            ("4", Outcome::Deadline),
            ("14", Outcome::Error(Some(tonic::Code::Unavailable))),
            ("1", Outcome::Cancelled),
        ] {
            headers.insert("grpc-status", code.parse().unwrap());
            assert_eq!(status(&headers), Some(outcome));
        }
    }

    #[derive(Default)]
    struct Recorder(std::sync::Mutex<Vec<Outcome>>);
    impl super::super::observation::Observer for Recorder {
        fn started(&self, _: Operation) {}

        fn finished(&self, _: Operation, _: std::time::Duration, outcome: Outcome) {
            self.0.lock().unwrap().push(outcome);
        }
    }

    struct Frames(std::collections::VecDeque<http_body::Frame<tonic::codegen::Bytes>>);
    impl http_body::Body for Frames {
        type Data = tonic::codegen::Bytes;
        type Error = tonic::Status;

        fn poll_frame(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            Poll::Ready(self.0.pop_front().map(Ok))
        }
    }

    fn observation(recorder: &'static Recorder) -> Observation {
        Observation::for_test(
            recorder,
            Operation::Rpc {
                method: RpcMethod::GetObject,
                traffic: Traffic::Normal,
            },
        )
    }

    #[tokio::test]
    async fn http_success_waits_for_rpc_trailers_and_records_once() {
        use http_body::Body as _;
        let recorder = Box::leak(Box::new(Recorder::default()));
        let mut trailers = http::HeaderMap::new();
        trailers.insert("grpc-status", "14".parse().unwrap());
        let body = Body::new(Frames(
            [
                http_body::Frame::data(tonic::codegen::Bytes::from_static(b"message")),
                http_body::Frame::trailers(trailers),
            ]
            .into(),
        ));
        let response = ResponseFuture {
            inner: std::future::ready(Ok::<_, tonic::Status>(Response::new(body))),
            guard: Some(observation(recorder)),
        }
        .await
        .unwrap();
        assert!(recorder.0.lock().unwrap().is_empty());
        let mut body = response.into_body();
        while std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .is_some()
        {}
        drop(body);
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [Outcome::Error(Some(tonic::Code::Unavailable))]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_response_wait_or_body_records_cancellation() {
        let recorder = Box::leak(Box::new(Recorder::default()));
        let response = ResponseFuture {
            inner: std::future::pending::<Result<Response<Body>, tonic::Status>>(),
            guard: Some(observation(recorder)),
        };
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), response)
                .await
                .is_err()
        );
        let body = ObservedBody {
            inner: Body::new(Frames(Default::default())),
            guard: Some(observation(recorder)),
        };
        drop(body);
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [Outcome::Cancelled, Outcome::Cancelled]
        );
    }

    #[tokio::test]
    async fn empty_response_without_grpc_status_is_an_error() {
        let recorder = Box::leak(Box::new(Recorder::default()));
        let response = ResponseFuture {
            inner: std::future::ready(Ok::<_, tonic::Status>(Response::new(Body::empty()))),
            guard: Some(observation(recorder)),
        }
        .await
        .unwrap();
        drop(response);
        assert_eq!(*recorder.0.lock().unwrap(), [Outcome::Error(None)]);
    }
}
