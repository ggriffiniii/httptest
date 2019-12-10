use crate::mappers::Matcher;
use crate::responders::Responder;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// type alias for a request that has read a complete body into memory.
type FullRequest = hyper::Request<hyper::body::Bytes>;

/// The Server
pub struct Server {
    trigger_shutdown: Option<futures::channel::oneshot::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
    addr: SocketAddr,
    state: ServerState,
}

impl Server {
    /// Start a server.
    ///
    /// The server will run in the background. On Drop it will terminate and
    /// assert it's expectations.
    pub fn run() -> Self {
        use futures::future::FutureExt;
        use hyper::{
            service::{make_service_fn, service_fn},
            Error,
        };
        let bind_addr = ([127, 0, 0, 1], 0).into();
        // And a MakeService to handle each connection...
        let state = ServerState::default();
        let make_service = make_service_fn({
            let state = state.clone();
            move |_| {
                let state = state.clone();
                async move {
                    let state = state.clone();
                    Ok::<_, Error>(service_fn({
                        let state = state.clone();
                        move |req: hyper::Request<hyper::Body>| {
                            let state = state.clone();
                            async move {
                                // read the full body into memory prior to handing it to mappers.
                                let (head, body) = req.into_parts();
                                let full_body = hyper::body::to_bytes(body).await?;
                                let req = hyper::Request::from_parts(head, full_body);
                                log::debug!("Received Request: {:?}", req);
                                let resp = on_req(state, req).await;
                                log::debug!("Sending Response: {:?}", resp);
                                hyper::Result::Ok(resp)
                            }
                        }
                    }))
                }
            }
        });
        // Then bind and serve...
        let server = hyper::Server::bind(&bind_addr).serve(make_service);
        let addr = server.local_addr();
        let (trigger_shutdown, shutdown_received) = futures::channel::oneshot::channel();
        let join_handle = std::thread::spawn(move || {
            let mut runtime = tokio::runtime::Builder::new()
                .basic_scheduler()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                futures::select! {
                    _ = server.fuse() => {},
                    _ = shutdown_received.fuse() => {},
                }
            });
        });

        Server {
            trigger_shutdown: Some(trigger_shutdown),
            join_handle: Some(join_handle),
            addr,
            state,
        }
    }

    /// Get the address the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get a fully formed url to the servers address.
    ///
    /// If the server is listening on port 1234.
    ///
    /// `server.url("/foo?q=1") == "http://localhost:1234/foo?q=1"`
    pub fn url<T>(&self, path_and_query: T) -> hyper::Uri
    where
        http::uri::PathAndQuery: std::convert::TryFrom<T>,
        <http::uri::PathAndQuery as std::convert::TryFrom<T>>::Error: Into<http::Error>,
    {
        hyper::Uri::builder()
            .scheme("http")
            .authority(format!("{}", &self.addr).as_str())
            .path_and_query(path_and_query)
            .build()
            .unwrap()
    }

    /// Add a new expectation to the server.
    pub fn expect(&self, expectation: Expectation) {
        self.state.push_expectation(expectation);
    }

    /// Verify all registered expectations. Panic if any are not met, then clear
    /// all expectations leaving the server running in a clean state.
    pub fn verify_and_clear(&mut self) {
        let mut state = self.state.lock();
        if std::thread::panicking() {
            // If the test is already panicking don't double panic on drop.
            state.expected.clear();
            return;
        }
        for expectation in state.expected.iter() {
            let is_valid_cardinality = match &expectation.cardinality {
                Times::AnyNumber => true,
                Times::AtLeast(lower_bound) if expectation.hit_count >= *lower_bound => true,
                Times::AtLeast(_) => false,
                Times::AtMost(limit) if expectation.hit_count <= *limit => true,
                Times::AtMost(_) => false,
                Times::Between(range)
                    if expectation.hit_count <= *range.end()
                        && expectation.hit_count >= *range.start() =>
                {
                    true
                }
                Times::Between(_) => false,
                Times::Exactly(limit) if expectation.hit_count == *limit => true,
                Times::Exactly(_) => false,
            };
            if !is_valid_cardinality {
                panic!(format!(
                    "Unexpected number of requests for matcher '{:?}'; received {}; expected {:?}",
                    &expectation.matcher, expectation.hit_count, &expectation.cardinality,
                ));
            }
        }
        state.expected.clear();
        if !state.unexpected_requests.is_empty() {
            // TODO: format and print the requests.
            panic!("unexpected requests received");
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // drop the trigger_shutdown channel to tell the server to shutdown.
        // Then wait for the shutdown to complete.
        self.trigger_shutdown = None;
        let _ = self.join_handle.take().unwrap().join();
        self.verify_and_clear();
    }
}

async fn on_req(state: ServerState, req: FullRequest) -> hyper::Response<hyper::Body> {
    let response_future = {
        let mut state = state.lock();
        let mut iter = state.expected.iter_mut();
        let response_future = loop {
            let expectation = match iter.next() {
                None => break None,
                Some(expectation) => expectation,
            };
            if expectation.matcher.matches(&req) {
                log::debug!("found matcher: {:?}", &expectation.matcher);
                expectation.hit_count += 1;
                let is_valid_cardinality = match &expectation.cardinality {
                    Times::AnyNumber => true,
                    Times::AtLeast(_) => true,
                    Times::AtMost(limit) if expectation.hit_count <= *limit => true,
                    Times::AtMost(_) => false,
                    Times::Between(range) if expectation.hit_count <= *range.end() => true,
                    Times::Between(_) => false,
                    Times::Exactly(limit) if expectation.hit_count <= *limit => true,
                    Times::Exactly(_) => false,
                };
                if is_valid_cardinality {
                    break Some(expectation.responder.respond());
                } else {
                    break Some(Box::pin(cardinality_error(
                        &*expectation.matcher as &dyn Matcher<FullRequest>,
                        &expectation.cardinality,
                        expectation.hit_count,
                    )));
                }
            }
        };
        if response_future.is_none() {
            // TODO: provide real request id.
            state.unexpected_requests.push(RequestID(1));
        }
        response_future
    };
    if let Some(f) = response_future {
        f.await
    } else {
        hyper::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body(hyper::Body::empty())
            .unwrap()
    }
}

/// How many requests should an expectation receive.
#[derive(Debug, Clone)]
pub enum Times {
    /// Allow any number of requests.
    AnyNumber,
    /// Require that at least this many requests are received.
    AtLeast(usize),
    /// Require that no more than this many requests are received.
    AtMost(usize),
    /// Require that the number of requests received is within this range.
    Between(std::ops::RangeInclusive<usize>),
    /// Require that exactly this many requests are received.
    Exactly(usize),
}

/// An expectation to be asserted by the server.
pub struct Expectation {
    matcher: Box<dyn Matcher<FullRequest>>,
    cardinality: Times,
    responder: Box<dyn Responder>,
    hit_count: usize,
}

impl Expectation {
    /// What requests will this expectation match.
    pub fn matching(matcher: impl Matcher<FullRequest> + 'static) -> ExpectationBuilder {
        ExpectationBuilder {
            matcher: Box::new(matcher),
            cardinality: Times::Exactly(1),
        }
    }
}

/// Define expectations using a builder pattern.
pub struct ExpectationBuilder {
    matcher: Box<dyn Matcher<FullRequest>>,
    cardinality: Times,
}

impl ExpectationBuilder {
    /// How many requests should this expectation receive.
    pub fn times(self, cardinality: Times) -> ExpectationBuilder {
        ExpectationBuilder {
            cardinality,
            ..self
        }
    }

    /// What should this expectation respond with.
    pub fn respond_with(self, responder: impl Responder + 'static) -> Expectation {
        Expectation {
            matcher: self.matcher,
            cardinality: self.cardinality,
            responder: Box::new(responder),
            hit_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RequestID(u64);

#[derive(Clone)]
struct ServerState(Arc<Mutex<ServerStateInner>>);

impl ServerState {
    fn lock(&self) -> std::sync::MutexGuard<ServerStateInner> {
        self.0.lock().expect("mutex poisoned")
    }

    fn push_expectation(&self, expectation: Expectation) {
        let mut inner = self.lock();
        inner.expected.push(expectation);
    }
}

impl Default for ServerState {
    fn default() -> Self {
        ServerState(Default::default())
    }
}

struct ServerStateInner {
    unexpected_requests: Vec<RequestID>,
    expected: Vec<Expectation>,
}

impl Default for ServerStateInner {
    fn default() -> Self {
        ServerStateInner {
            unexpected_requests: Default::default(),
            expected: Default::default(),
        }
    }
}

fn cardinality_error(
    matcher: &dyn Matcher<FullRequest>,
    cardinality: &Times,
    hit_count: usize,
) -> Pin<Box<dyn Future<Output = hyper::Response<hyper::Body>> + Send + 'static>> {
    let body = hyper::Body::from(format!(
        "Unexpected number of requests for matcher '{:?}'; received {}; expected {:?}",
        matcher, hit_count, cardinality,
    ));
    Box::pin(async move {
        hyper::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body(body)
            .unwrap()
    })
}
