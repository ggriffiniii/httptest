use crate::matchers::{matcher_name, ExecutionContext, Matcher};
use crate::responders::Responder;
use futures::future::FutureExt;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::service::service_fn;
use hyper_util::{rt::TokioIo, server::conn::auto::Builder};
use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::net::{SocketAddr, TcpListener};
use std::ops::{Bound, RangeBounds};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// type alias for a request that has read a complete body into memory.
type FullRequest = http::Request<hyper::body::Bytes>;

/// The Server
#[derive(Debug)]
pub struct Server {
    trigger_shutdown: Option<tokio::sync::watch::Sender<bool>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
    addr: SocketAddr,
    state: ServerState,
}

impl Server {
    /// Start a server, panicking if unable to start.
    ///
    /// The server will run in the background. On Drop it will terminate and
    /// assert it's expectations.
    pub fn run() -> Self {
        ServerBuilder::new().run().unwrap()
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
    pub fn url(&self, path_and_query: &str) -> http::Uri {
        hyper::Uri::builder()
            .scheme("http")
            .authority(self.addr.to_string().as_str())
            .path_and_query(path_and_query)
            .build()
            .unwrap()
    }

    /// Get a fully formed url to the servers address as a String.
    ///
    /// `server.url_str(foo)  == server.url(foo).to_string()`
    pub fn url_str(&self, path_and_query: &str) -> String {
        self.url(path_and_query).to_string()
    }

    /// Add a new expectation to the server.
    pub fn expect(&self, expectation: Expectation) {
        log::debug!("expectation added: {:?}", expectation);
        self.state.push_expectation(expectation);
    }

    /// Verify all registered expectations. Panic if any are not met, then clear
    /// all expectations leaving the server running in a clean state.
    pub fn verify_and_clear(&mut self) {
        let state = {
            let mut state = self.state.lock().expect("mutex poisoned");
            std::mem::take(&mut *state) // reset server to default state.
        };
        if std::thread::panicking() {
            // Since unexpected requests are always a mistake in these engineered test
            // scenarios, if any occur, we should yell about them as they are either an
            // underlying cause of the original panic and test failure or the test isn't
            // accounting for every aspect and needs to be updated.
            //
            // We can't double panic! because it will lead to an immediate termination
            // with an ugly backtrace. But since we're already panicking, we get the
            // same effect by just printing what we would have put in the panic message.
            if !state.unexpected_requests.is_empty() {
                println!(
                    "Received the following unexpected requests: {:#?}",
                    &state.unexpected_requests
                );
            }

            // If the test is already panicking don't double panic on drop.
            //
            // Additionally, we don't want the noise from the matchers that failed just
            // because the test only got half-way through.
            return;
        }
        for expectation in state.expected.iter() {
            if !hit_count_is_valid(expectation.times, expectation.hit_count) {
                let unexpected_requests_message = if state.unexpected_requests.is_empty() {
                    "(no other unexpected requests)".to_string()
                } else {
                    format!(
                        "There were {} other unexpected requests that you may have expected to match: {:#?}",
                        state.unexpected_requests.len(),
                        &state.unexpected_requests,
                    )
                };

                panic!(
                    "Unexpected number of requests for matcher '{:?}'; received {}; expected {}. {}",
                    matcher_name(&*expectation.matcher),
                    expectation.hit_count,
                    RangeDisplay(expectation.times),
                    unexpected_requests_message,
                );
            }
        }
        if !state.unexpected_requests.is_empty() {
            panic!(
                "Received the following unexpected requests:\n{:#?}",
                &state.unexpected_requests
            );
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

async fn process_request(
    state: ServerState,
    req: hyper::Request<hyper::body::Incoming>,
) -> hyper::Result<http::Response<BoxBody<hyper::body::Bytes, Infallible>>> {
    // read the full body into memory prior to handing it to matchers.
    let (head, body) = req.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let req = http::Request::from_parts(head, bytes);

    log::debug!("Received Request: {:?}", req);
    let resp = on_req(state, req).await;

    let (parts, body) = resp.into_parts();
    let body = Full::new(body).boxed();
    let resp = hyper::Response::from_parts(parts, body);

    log::debug!("Sending Response: {:?}", resp);
    hyper::Result::Ok(resp)
}

async fn on_req(state: ServerState, req: FullRequest) -> http::Response<hyper::body::Bytes> {
    let response_future = {
        let mut state = state.lock().expect("mutex poisoned");
        // Iterate over expectations in reverse order. Expectations are
        // evaluated most recently added first.
        match state.find_expectation(&req) {
            Some(expectation) => {
                log::debug!("found matcher: {:?}", matcher_name(&*expectation.matcher));
                expectation.hit_count += 1;
                if !times_exceeded(expectation.times.1, expectation.hit_count) {
                    Some(expectation.responder.respond(&req))
                } else {
                    Some(times_error(
                        &*expectation.matcher as &dyn Matcher<FullRequest>,
                        expectation.times,
                        expectation.hit_count,
                    ))
                }
            }
            None => {
                log::debug!("no matcher found for request: {:?}", req);
                state.unexpected_requests.push(req);
                None
            }
        }
    };
    if let Some(f) = response_future {
        f.await
    } else {
        http::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body("No matcher found".into())
            .unwrap()
    }
}

fn times_exceeded(end_bound: Bound<usize>, hit_count: usize) -> bool {
    match end_bound {
        Bound::Included(limit) if hit_count > limit => true,
        Bound::Excluded(limit) if hit_count >= limit => true,
        _ => false,
    }
}

fn hit_count_is_valid(bounds: (Bound<usize>, Bound<usize>), hit_count: usize) -> bool {
    bounds.contains(&hit_count)
}

/// An expectation to be asserted by the server.
pub struct Expectation {
    matcher: Box<dyn Matcher<FullRequest>>,
    times: (Bound<usize>, Bound<usize>),
    responder: Box<dyn Responder>,
    hit_count: usize,
}

impl Expectation {
    /// What requests will this expectation match.
    pub fn matching(matcher: impl Matcher<FullRequest> + 'static) -> ExpectationBuilder {
        ExpectationBuilder {
            matcher: Box::new(matcher),
            // expect exactly one request by default.
            times: (Bound::Included(1), Bound::Included(1)),
        }
    }
}

impl fmt::Debug for Expectation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Expectation")
            .field("matcher", &matcher_name(&*self.matcher))
            .field("times", &self.times)
            .field("hit_count", &self.hit_count)
            .finish()
    }
}

/// Define expectations using a builder pattern.
pub struct ExpectationBuilder {
    matcher: Box<dyn Matcher<FullRequest>>,
    times: (Bound<usize>, Bound<usize>),
}

impl ExpectationBuilder {
    /// Expect this many requests.
    ///
    /// ```
    /// # use httptest::{Expectation, matchers::any, responders::status_code};
    /// // exactly 2 requests
    /// Expectation::matching(any()).times(2).respond_with(status_code(200));
    /// // at least 2 requests
    /// Expectation::matching(any()).times(2..).respond_with(status_code(200));
    /// // at most 2 requests
    /// Expectation::matching(any()).times(..=2).respond_with(status_code(200));
    /// // between 2 and 5 inclusive
    /// Expectation::matching(any()).times(2..6).respond_with(status_code(200));
    /// // equivalently
    /// Expectation::matching(any()).times(2..=5).respond_with(status_code(200));
    /// ```
    pub fn times<R>(self, times: R) -> ExpectationBuilder
    where
        R: crate::into_times::IntoTimes,
    {
        ExpectationBuilder {
            times: times.into_times(),
            ..self
        }
    }

    /// What should this expectation respond with.
    pub fn respond_with(self, responder: impl Responder + 'static) -> Expectation {
        Expectation {
            matcher: self.matcher,
            times: self.times,
            responder: Box::new(responder),
            hit_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct ServerState(Arc<Mutex<ServerStateInner>>);

impl ServerState {
    fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, ServerStateInner>> {
        self.0.lock()
    }

    fn push_expectation(&self, expectation: Expectation) {
        let mut inner = self.lock().expect("mutex poisoned");
        inner.expected.push(expectation);
    }
}

impl Default for ServerState {
    fn default() -> Self {
        ServerState(Default::default())
    }
}

#[derive(Debug)]
struct ServerStateInner {
    unexpected_requests: Vec<FullRequest>,
    expected: Vec<Expectation>,
}

impl ServerStateInner {
    fn find_expectation(&mut self, req: &FullRequest) -> Option<&mut Expectation> {
        for expectation in self.expected.iter_mut().rev() {
            if ExecutionContext::evaluate(expectation.matcher.as_mut(), req) {
                return Some(expectation);
            }
        }
        None
    }
}

impl Default for ServerStateInner {
    fn default() -> Self {
        ServerStateInner {
            unexpected_requests: Default::default(),
            expected: Default::default(),
        }
    }
}

fn times_error(
    matcher: &dyn Matcher<FullRequest>,
    times: (Bound<usize>, Bound<usize>),
    hit_count: usize,
) -> Pin<Box<dyn Future<Output = http::Response<hyper::body::Bytes>> + Send + 'static>> {
    let body = hyper::body::Bytes::from(format!(
        "Unexpected number of requests for matcher '{:?}'; received {}; expected {}",
        matcher_name(&*matcher),
        hit_count,
        RangeDisplay(times),
    ));
    Box::pin(async move {
        http::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body(body)
            .unwrap()
    })
}

struct RangeDisplay((Bound<usize>, Bound<usize>));
impl fmt::Display for RangeDisplay {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // canonicalize the bounds to inclusive or unbounded.
        enum MyBound {
            Included(usize),
            Unbounded,
        }
        let inclusive_start = match (self.0).0 {
            Bound::Included(x) => MyBound::Included(x),
            Bound::Excluded(x) => MyBound::Included(x + 1),
            Bound::Unbounded => MyBound::Unbounded,
        };
        let inclusive_end = match (self.0).1 {
            Bound::Included(x) => MyBound::Included(x),
            Bound::Excluded(x) => MyBound::Included(x - 1),
            Bound::Unbounded => MyBound::Unbounded,
        };
        match (inclusive_start, inclusive_end) {
            (MyBound::Included(min), MyBound::Unbounded) => write!(f, "AtLeast({})", min),
            (MyBound::Unbounded, MyBound::Included(max)) => write!(f, "AtMost({})", max),
            (MyBound::Included(min), MyBound::Included(max)) if min == max => {
                write!(f, "Exactly({})", max)
            }
            (MyBound::Included(min), MyBound::Included(max)) => {
                write!(f, "Between({}..={})", min, max)
            }
            (MyBound::Unbounded, MyBound::Unbounded) => write!(f, "Any"),
        }
    }
}

/// Custom Server Builder.
pub struct ServerBuilder {
    bind_addr: Option<SocketAddr>,
}

impl ServerBuilder {
    /// Create a new ServerBuilder. By default the server will listen on ipv6
    /// loopback if available and fallback to ipv4 loopback if unable to bind to
    /// ipv6.
    pub fn new() -> ServerBuilder {
        ServerBuilder { bind_addr: None }
    }

    /// Specify the address the server should listen on.
    pub fn bind_addr(self, bind_addr: SocketAddr) -> ServerBuilder {
        ServerBuilder {
            bind_addr: Some(bind_addr),
        }
    }

    /// Start a server.
    ///
    /// The server will run in the background. On Drop it will terminate and
    /// assert it's expectations.
    pub fn run(self) -> std::io::Result<Server> {
        // And a MakeService to handle each connection...
        let state = ServerState::default();
        let service = |state: ServerState| {
            service_fn(move |req: http::Request<hyper::body::Incoming>| {
                let state = state.clone();
                process_request(state, req)
            })
        };

        let listener = Self::listener(self.bind_addr)?;
        listener.set_nonblocking(true)?;

        let addr = listener.local_addr()?;

        // Then bind and serve...
        let (trigger_shutdown, mut shutdown_received) = tokio::sync::watch::channel(false);
        let state_listener = state.clone();
        let join_handle = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap();

            runtime.block_on(async move {
                let mut connection_tasks = tokio::task::JoinSet::new();
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let conn_shutdown_receiver = shutdown_received.clone();

                let server = async {
                    loop {
                        let (stream, _addr) = match listener.accept().await {
                            Ok(a) => a,
                            Err(e) => {
                                panic!("listener failed to accept a new connection: {}", e);
                            }
                        };

                        let state_c = state_listener.clone();
                        let mut conn_shutdown_receiver_c = conn_shutdown_receiver.clone();
                        connection_tasks.spawn(async move {
                            let builder = Builder::new(hyper_util::rt::TokioExecutor::new());
                            let connection = builder
                                .serve_connection(TokioIo::new(stream), service(state_c.clone()));
                            tokio::pin!(connection);

                            tokio::select! {
                                _ = connection.as_mut() => {}
                                _ = conn_shutdown_receiver_c.changed().fuse() => {
                                    connection.as_mut().graceful_shutdown()
                                }
                            };
                        });
                    }
                };

                tokio::select! {
                    _ = server.fuse() => {},
                    _ = shutdown_received.changed().fuse() => {},
                }

                while (connection_tasks.join_next().await).is_some() {}
            });
        });

        Ok(Server {
            trigger_shutdown: Some(trigger_shutdown),
            join_handle: Some(join_handle),
            addr,
            state,
        })
    }

    fn listener(bind_addr: Option<SocketAddr>) -> std::io::Result<TcpListener> {
        match bind_addr {
            Some(addr) => TcpListener::bind(addr),
            None => {
                let ipv6_bind_addr: SocketAddr = ([0, 0, 0, 0, 0, 0, 0, 1], 0).into();
                let ipv4_bind_addr: SocketAddr = ([127, 0, 0, 1], 0).into();
                TcpListener::bind(ipv6_bind_addr).or_else(|_| TcpListener::bind(ipv4_bind_addr))
            }
        }
    }
}
