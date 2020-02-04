/*!
# httptest

Provide convenient mechanism for testing http clients against a locally
running http server. The typical usage is as follows:

* Start a server
* Configure the server by adding expectations
* Test your http client by making requests to the server
* On Drop the server verifies all expectations were met.

## Example Test

```
# async fn foo() {
use httptest::{Server, Expectation, mappers::*, responders::*};
// Start a server running on a local ephemeral port.
let server = Server::run();
// Configure the server to expect a single GET /foo request and respond
// with a 200 status code.
server.expect(
    Expectation::matching(request::method_path("GET", "/foo"))
    .respond_with(status_code(200)),
);

// The server provides server.addr() that returns the address of the
// locally running server, or more conveniently provides a server.url() method
// that gives a fully formed http url to the provided path.
let url = server.url("/foo");
let client = hyper::Client::new();
// Issue the GET /foo to the server.
let resp = client.get(url).await.unwrap();

// assert the response has a 200 status code.
assert!(resp.status().is_success());

// on Drop the server will assert all expectations have been met and will
// panic if not.
# }
```

# Server behavior

Typically the server is started by calling
[Server::run](struct.Server.html#method.run). It starts without any
expectations configured.

Expectations are added by calling
[Server::expect](struct.Server.html#method.expect). Every invocation of
expect appends a new expectation onto the list. Expectations are only removed
from the server on Drop or when
[Server::verify_and_clear](struct.Server.html#method.verify_and_clear) is
invoked. This guarantees that all expectations are always verified.

Expectations consist of:
* A matcher that determines which requests match this expectation
* The number of times a request matching this expectation is expected to be received
* A responder that indicates how the server should respond to the request.

When the server receives a request it iterates over all expectations in the
*reverse* order they have been added. When it reaches an expectation that
matches the request, it increments the hit count on that expectation and
verifies it has not exceeded it's expected number of requests. If the
limit has been exceeded a 500 error is returned, if the limit has not been
exceeded it uses the expectation's responder to respond to the request. If the
request does not match any expectation a 500 error is returned.

When the server is Dropped it:
* Stops running
* Panics if
  * any expectation did not receive the expected number of requests
  * a request was received that did not match any expectation

Clients can determine the address and port the server is reachable at using
[Server::addr](struct.Server.html#method.addr), or the helper methods
[Server::url](struct.Server.html#method.url) and
[Server::url_str](struct.Server.html#method.url_str).

## Server Pooling

Typical usage would use [Server::run](struct.Server.html#method.run) early in
each test case and have the Drop implementation at the end of the test assert
all expectations were met. This runs a separate server for each test. Rust's
test harness starts a separate thread for each test within a test-suite so
the machine running the test would likely end up running a server for each
#[test] function concurrently. For large test suites this could cause machine
wide resources (like tcp ports) to become scarce. To address this you could
use the --test-threads flag on the test-harness to limit the number of
threads running, or alternatively you could use a global
[ServerPool](struct.ServerPool.html) instance.

The [ServerPool](struct.ServerPool.html) allows limiting the number of
servers that can be running concurrently while still allowing test cases to
function independently.

### ServerPool example

```
# use httptest::ServerPool;
// Create a server pool that will create at most 2 servers.
static SERVER_POOL: ServerPool = ServerPool::new(2);

#[test]
fn test1() {
    let server = SERVER_POOL.get_server();
    server.Expect(Expectation::matching(any()).respond_with(status_code(200)));

    // Send requests to server
    // Server will assert expectations on drop.
}

#[test]
fn test2() {
    let server = SERVER_POOL.get_server();
    server.Expect(Expectation::matching(any()).respond_with(status_code(200)));

    // Send requests to server
    // Server will assert expectations on drop.
}

#[test]
fn test3() {
    let server = SERVER_POOL.get_server();
    server.Expect(Expectation::matching(any()).respond_with(status_code(200)));

    // Send requests to server
    // Server will assert expectations on drop.
}
```

This is almost identical to tests without pooling, the only addition is
creating a static ServerPool instance, and using `SERVER_POOL.get_server()`
instead of `Server::run()`. This will effectively limit the amount of
concurrency of the test suite to two tests at a time. The first two tests to execute
`get_server()` will be handed servers without blocking, the 3rd test will block
in `get_server()` until one of the first 2 tests complete.

# Defining Expecations

Every expecation defines a request matcher, a defintion of the number of
times it's expected to be called, and what it should respond with.

### Expectation example

```
use httptest::{Expectation, mappers::*, responders::*};

// Define an Expectation that matches any request to path /foo, expects to
// receive at least 1 such request, and responds with a 200 response.
Expectation::matching(request::path("/foo"))
    .times(1..)
    .respond_with(status_code(200));
```

## Request Matchers

Defining which request an expecation matches is done in a composable manner
using a series of traits. The core of which is
[Mapper](mappers/trait.Mapper.html). The `Mapper` trait is generic
over an input type, has an associated `Out` type, and defines a single method
`map` that converts from a shared reference of the input type to the `Out`
type.

A request matcher is any `Mapper` that accepts a
`http::Request<hyper::body::Bytes>` as input, and maps that to a boolean. A
true result indicates the request matches.

With that understanding we can discuss how to easily define a request
matcher. There are a variety of pre-defined mappers within the
[mappers](mappers/index.html) module. These mappers can be composed
together to define the values you want to match. The mappers fall into two
categories. Some of the mappers extract a value from the input type and pass
it to another mapper, other mappers accept an input type and return a bool.
These primitives provide an easy and flexible way to define custom logic.

### Matcher examples

```
// pull all the predefined mappers into our namespace.
use httptest::mappers::*;

// &str, String, and &[u8] all implement mappers that test for equality.
// All of these mappers return true when the input equals "/foo"
let mut m = eq("/foo");
let mut m = "/foo";
let mut m = "/foo".to_string();
let mut m = &b"/foo"[..];

// A mapper that returns true when the input matches the regex "(foo|bar).*"
let mut m = matches("(foo|bar).*");

// A request matcher that matches a request to path "/foo"
let mut m = request::path("/foo");

// A request matcher that matches a POST request
let mut m = request::method("POST");

// A request matcher that matches a POST with a path that matches the regex 'foo.*'
let mut m = all_of![
    request::method("POST"),
    request::path(matches("foo.*")),
];

# // Allow type inference to determine the request type.
# m.map(&http::Request::get("/").body("").unwrap());
```

## Times

Each expectation defines how many times a matching request is expected to
be received. The default is exactly once. The ExpectationBuilder provides a
[times](struct.ExpectationBuilder.html#method.times) method to specify the
number of requests expected.

```
# use httptest::{Expectation, mappers::any, responders::status_code};
// Expect exactly one request
Expectation::matching(any())
    .respond_with(status_code(200));

// Expect exactly two requests
Expectation::matching(any())
    .times(2)
    .respond_with(status_code(200));

// Expect at least 2 requests
Expectation::matching(any())
    .times(2..)
    .respond_with(status_code(200));

// Expect at most 2 requests
Expectation::matching(any())
    .times(..2)
    .respond_with(status_code(200));

// Expect between 2 and 5 requests
Expectation::matching(any())
    .times(2..6)
    .respond_with(status_code(200));

// Expect between 2 and 5 requests
Expectation::matching(any())
    .times(2..=5)
    .respond_with(status_code(200));

// Expect any number of requests.
Expectation::matching(any())
    .times(..)
    .respond_with(status_code(200));
```

The server will respond to any requests that violate the times restriction with
a 500 status code and the server will subsequently panic on Drop.

## Responder

Responders define how the server will respond to a matched request. There
are a number of implemented responders within the responders module. In
addition to the predefined responders you can provide any
`http::Response` with a body that can be cloned or implement your own
Responder.

### Responder example

```
use httptest::responders::*;

// respond with a successful 200 status code.
status_code(200);

// respond with a 404 page not found and a custom header.
status_code(404).append_header("X-My-Hdr", "my hdr val");

// respond with a successful 200 status code and body.
status_code(200).body("my body");

// respond with a json encoded body and custom header.
json_encoded(serde_json::json!({
    "my_key": 100,
    "my_key2": [1, 2, "foo", 99],
})).append_header("X-My-Hdr", "my hdr val");

// respond with a url encoded body (foo=bar&baz=bat)
url_encoded(&[
    ("foo", "bar"),
    ("baz", "bat")
]);

// alternate between responding with a 200 and a 404.
cycle![
    status_code(200),
    status_code(404),
];
```
!*/

#![deny(missing_docs)]

/// true if all the provided matchers return true.
///
/// The macro exists to conveniently box a list of mappers and put them into a
/// `Vec<Box<dyn Mapper>>`. The translation is:
///
/// `all_of![a, b] => all_of(vec![Box::new(a), Box::new(b)])`
#[macro_export]
macro_rules! all_of {
    ($($x:expr),*) => ($crate::mappers::all_of($crate::vec_of_boxes![$($x),*]));
    ($($x:expr,)*) => ($crate::all_of![$($x),*]);
}

/// true if any of the provided matchers return true.
///
/// The macro exists to conveniently box a list of mappers and put them into a
/// `Vec<Box<dyn Mapper>>`. The translation is:
///
/// `any_of![a, b] => any_of(vec![Box::new(a), Box::new(b)])`
#[macro_export]
macro_rules! any_of {
    ($($x:expr),*) => ($crate::mappers::any_of($crate::vec_of_boxes![$($x),*]));
    ($($x:expr,)*) => ($crate::any_of![$($x),*]);
}

/// a Responder that cycles through a list of responses.
///
/// The macro exists to conveniently box a list of responders and put them into a
/// `Vec<Box<dyn Responder>>`. The translation is:
///
/// `cycle![a, b] => cycle(vec![Box::new(a), Box::new(b)])`
#[macro_export]
macro_rules! cycle {
    ($($x:expr),*) => ($crate::responders::cycle($crate::vec_of_boxes![$($x),*]));
    ($($x:expr,)*) => ($crate::cycle![$($x),*]);
}

// hidden from docs because it's an implementation detail of the above macros.
#[doc(hidden)]
#[macro_export]
macro_rules! vec_of_boxes {
    ($($x:expr),*) => (std::vec![$(std::boxed::Box::new($x)),*]);
    ($($x:expr,)*) => ($crate::vec_of_boxes![$($x),*]);
}

mod into_times;
pub mod mappers;
pub mod responders;
mod server;
mod server_pool;

pub use server::{Expectation, ExpectationBuilder, Server};
pub use server_pool::{ServerHandle, ServerPool};
