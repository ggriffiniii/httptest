use crate::mappers::Matcher;
use crate::responders::Responder;
use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::ops::{Bound, RangeBounds};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// type alias for a request that has read a complete body into memory.
type FullRequest = http::Request<hyper::body::Bytes>;

/// The Server
#[derive(Debug)]
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
                        move |req: http::Request<hyper::Body>| {
                            let state = state.clone();
                            async move {
                                // read the full body into memory prior to handing it to mappers.
                                let (head, body) = req.into_parts();
                                let full_body = hyper::body::to_bytes(body).await?;
                                let req = http::Request::from_parts(head, full_body);
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
        let (addr_tx, addr_rx) = crossbeam_channel::unbounded();
        // Then bind and serve...
        let (trigger_shutdown, shutdown_received) = futures::channel::oneshot::channel();
        let join_handle = std::thread::spawn(move || {
            let mut runtime = tokio::runtime::Builder::new()
                .basic_scheduler()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let server = hyper::Server::bind(&bind_addr).serve(make_service);
                addr_tx.send(server.local_addr()).unwrap();
                futures::select! {
                    _ = server.fuse() => {},
                    _ = shutdown_received.fuse() => {},
                }
            });
        });
        let addr = addr_rx.recv().unwrap();
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
        let mut state = self.state.lock();
        if std::thread::panicking() {
            // If the test is already panicking don't double panic on drop.
            state.expected.clear();
            return;
        }
        for expectation in state.expected.iter() {
            if !hit_count_is_valid(expectation.times, expectation.hit_count) {
                panic!(format!(
                    "Unexpected number of requests for matcher '{:?}'; received {}; expected {}",
                    &expectation.matcher, expectation.hit_count, RangeDisplay(expectation.times),
                ));
            }
        }
        if state.unexpected_requests != 0 {
            panic!("{} unexpected requests received", state.unexpected_requests);
        }
        // reset the server back to default state.
        *state = ServerStateInner::default();
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

async fn on_req(state: ServerState, req: FullRequest) -> http::Response<hyper::Body> {
    let response_future = {
        let mut state = state.lock();
        // Iterate over expectations in reverse order. Expectations are
        // evaluated most recently added first.
        let mut iter = state.expected.iter_mut().rev();
        let response_future = loop {
            let expectation = match iter.next() {
                None => break None,
                Some(expectation) => expectation,
            };
            if expectation.matcher.matches(&req) {
                log::debug!("found matcher: {:?}", &expectation.matcher);
                expectation.hit_count += 1;
                if !times_exceeded(expectation.times.1, expectation.hit_count) {
                    break Some(expectation.responder.respond());
                } else {
                    break Some(Box::pin(times_error(
                        &*expectation.matcher as &dyn Matcher<FullRequest>,
                        expectation.times,
                        expectation.hit_count,
                    )));
                }
            }
        };
        if response_future.is_none() {
            log::debug!("no matcher found for request: {:?}", req);
            state.unexpected_requests += 1;
        }
        response_future
    };
    if let Some(f) = response_future {
        f.await
    } else {
        http::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body(hyper::Body::from("No matcher found"))
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
#[derive(Debug)]
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

/// Define expectations using a builder pattern.
pub struct ExpectationBuilder {
    matcher: Box<dyn Matcher<FullRequest>>,
    times: (Bound<usize>, Bound<usize>),
}

impl ExpectationBuilder {
    /// Expect this many requests.
    ///
    /// ```
    /// # use httptest::{Expectation, mappers::any, responders::status_code};
    /// // exactly 2 requests
    /// Expectation::matching(any()).times(2..=2).respond_with(status_code(200));
    /// // at least 2 requests
    /// Expectation::matching(any()).times(2..).respond_with(status_code(200));
    /// // at most 2 requests
    /// Expectation::matching(any()).times(..=2).respond_with(status_code(200));
    /// // between 2 and 5 inclusive
    /// Expectation::matching(any()).times(2..6).respond_with(status_code(200));
    /// // equivalently
    /// Expectation::matching(any()).times(2..=5).respond_with(status_code(200));
    /// ```
    pub fn times<R>(self, range: R) -> ExpectationBuilder
    where
        R: RangeBounds<usize>,
    {
        fn cloned_range(b: Bound<&usize>) -> Bound<usize> {
            match b {
                Bound::Included(b) => Bound::Included(*b),
                Bound::Excluded(b) => Bound::Excluded(*b),
                Bound::Unbounded => Bound::Unbounded,
            }
        }
        let start_bound = cloned_range(range.start_bound());
        let end_bound = cloned_range(range.end_bound());
        ExpectationBuilder {
            times: (start_bound, end_bound),
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

#[derive(Debug)]
struct ServerStateInner {
    unexpected_requests: usize,
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

fn times_error(
    matcher: &dyn Matcher<FullRequest>,
    times: (Bound<usize>, Bound<usize>),
    hit_count: usize,
) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'static>> {
    let body = hyper::Body::from(format!(
        "Unexpected number of requests for matcher '{:?}'; received {}; expected {}",
        matcher,
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
