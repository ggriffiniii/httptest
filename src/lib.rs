//! # httptest
//!
//! Provide convenient mechanism for testing http clients against a locally
//! running http server. The typical usage is as follows:
//!
//! * Start a server
//! * Configure the server by adding expectations
//! * Test your http client by making requests to the server
//! * On Drop the server verifies all expectations were met.
//!
//! ## Example Test
//!
//! ```
//! # async fn foo() {
//! use httptest::{Server, Expectation, Times, mappers::*, responders::*};
//! // Start a server running on a local ephemeral port.
//! let server = Server::run();
//! // Configure the server to expect a single GET /foo request and respond
//! // with a 200 status code.
//! server.expect(
//!     Expectation::matching(all_of![
//!         request::method("GET"),
//!         request::path("/foo")
//!     ])
//!     .times(Times::Exactly(1))
//!     .respond_with(status_code(200)),
//! );
//!
//! // The server provides server.addr() that returns the address of the
//! // locally running server, or more conveniently provides a server.url() method
//! // that gives a fully formed http url to the provided path.
//! let url = server.url("/foo");
//! let client = hyper::Client::new();
//! // Issue the GET /foo to the server.
//! let resp = client.get(url).await.unwrap();
//!
//! // Use response matchers to assert the response has a 200 status code.
//! assert!(response::status_code(eq(200)).matches(&resp));
//!
//! // on Drop the server will assert all expectations have been met and will
//! // panic if not.
//! # }
//! ```
//!
//! # Server behavior
//!
//! The Server is started with [run()](struct.Server.html#method.run).
//!
//! The server will run in a background thread until it's dropped. Once dropped
//! it will assert that every configured expectation has been met or will panic.
//! You can also use [verify_and_clear()](struct.Server.html#method.verify_and_clear)
//! to assert and clear the expectations while keeping the server running.
//!
//! [addr()](struct.Server.html#method.addr) will return the address the
//! server is listening on.
//!
//! [url()](struct.Server.html#method.url) will
//! construct a fully formed http url to the path provided i.e.
//!
//! `server.url("/foo?key=value") == "https://<server_addr>/foo?key=value"`.
//!
//! # Defining Expecations
//!
//! Every expecation defines a request matcher, a defintion of the number of
//! times it's expected to be called, and what it should respond with.
//!
//! ### Expectation example
//!
//! ```
//! use httptest::{Expectation, mappers::*, responders::*, Times};
//!
//! // Define an Expectation that matches any request to path /foo, expects to
//! // receive at least 1 such request, and responds with a 200 response.
//! Expectation::matching(request::path("/foo"))
//!     .times(Times::AtLeast(1))
//!     .respond_with(status_code(200));
//! ```
//!
//! ## Request Matchers
//!
//! Defining which request an expecation matches is done in a composable manner
//! using a series of traits. The core of which is
//! [Mapper](mappers/trait.Mapper.html). The `Mapper` trait is generic
//! over an input type, has an associated `Out` type, and defines a single method
//! `map` that converts from a shared reference of the input type to the `Out`
//! type.
//!
//! There's a specialized form of a Mapper where the `Out` type is a boolean.
//! Any `Mapper` that outputs a boolean value is considered a Matcher and
//! implements the [Matcher](mapper/trait.Matcher.html) trait as well. The
//! Matcher trait simply provides a `matches` method.
//!
//! A request matcher is any `Matcher` that takes accepts a
//! `http::Request<hyper::body::Bytes>` as input.
//!
//! With that understanding we can discuss how to easily define a request
//! matcher. There are a variety of pre-defined mappers within the
//! [mappers](mappers/index.html) module. These mappers can be composed
//! together to define the values you want to match. The mappers fall into two
//! categories. Some of the mappers extract a value from the input type and pass
//! it to another mapper, other mappers accept an input type and return a bool.
//! These primitives provide an easy and flexible way to define custom logic.
//!
//! ### Matcher examples
//!
//! ```
//! // pull all the predefined mappers into our namespace.
//! use httptest::mappers::*;
//!
//! // &str, String, and &[u8] all implement mappers that test for equality.
//! // All of these mappers return true when the input equals "/foo"
//! let mut m = eq("/foo");
//! let mut m = "/foo";
//! let mut m = "/foo".to_string();
//! let mut m = &b"/foo"[..];
//!
//! // A mapper that returns true when the input matches the regex "(foo|bar).*"
//! let mut m = matches("(foo|bar).*");
//!
//! // A request matcher that matches a request to path "/foo"
//! let mut m = request::path("/foo");
//!
//! // A request matcher that matches a POST request
//! let mut m = request::method("POST");
//!
//! // A request matcher that matches a POST with a path that matches the regex 'foo.*'
//! let mut m = all_of![
//!     request::method("POST"),
//!     request::path(matches("foo.*")),
//! ];
//!
//! # // Allow type inference to determine the request type.
//! # m.map(&http::Request::get("/").body("").unwrap());
//! ```
//!
//! ## Times
//!
//! Each expectation defines how many times a matching request is expected to
//! be received. The [Times](enum.Times.html) enum defines the possibility.
//! `Times::Exactly(1)` is the default value of an `Expectation` if one is not
//! specified with the
//! [times()](struct.ExpectationBuilder.html#method.times) method.
//!
//! The server will respond to any requests that violate the times request with
//! a 500 status code and the server will subsequently panic on Drop.
//!
//! ## Responder
//!
//! Responders define how the server will respond to a matched request. There
//! are a number of implemented responders within the responders module. In
//! addition to the predefined responders you can provide any
//! `http::Response` with a body that can be cloned or implement your own
//! Responder.
//!
//! ## Responder example
//!
//! ```
//! use httptest::responders::*;
//!
//! // respond with a successful 200 status code.
//! status_code(200);
//!
//! // respond with a 404 page not found.
//! status_code(404);
//!
//! // respond with a json encoded body.
//! json_encoded(serde_json::json!({
//!     "my_key": 100,
//!     "my_key2": [1, 2, "foo", 99],
//! }));
//!
//! // alternate between responding with a 200 and a 404.
//! cycle![
//!     status_code(200),
//!     status_code(404),
//! ];
//!
//! ```

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

pub mod mappers;
pub mod responders;
mod server;
mod server_pool;

pub use server::{Expectation, ExpectationBuilder, Server, Times};
pub use server_pool::{ServerHandle, ServerPool};
