//! Responder implementations.
//!
//! Reponders determine how the server will respond.
//!
//! Notable types that implement responder are
//! * `ResponseBuilder`
//!   * The `ResponseBuilder` can be constructed via the `status_code` function, and
//!     has convenience methods to modify the response further.
//! * `http::Response<String>` or `http::Response<Vec<u8>>`
//! * A function that returns a Responder.
//!   * The function is allowed to make arbitrary blocking calls like
//!     std::thread::sleep or reading from a file without impacting concurrent
//!     connections to the server.

use std::convert::TryInto;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

// import the cycle macro so that it's available if people glob import this module.
#[doc(inline)]
pub use crate::cycle;

/// Respond with an HTTP response.
pub trait Responder: Send {
    /// Return a future that outputs an HTTP response.
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::body::Bytes>> + Send + 'a>>;
}

/// Convenient ResponseBuilder that implements Responder.
#[derive(Debug)]
pub struct ResponseBuilder<B>(http::Response<B>);

impl<B> ResponseBuilder<B> {
    /// Set the http version.
    pub fn version(mut self, version: http::Version) -> Self {
        *self.0.version_mut() = version;
        self
    }

    /// Insert the provided header. Replacing any header that already exists with the same name.
    pub fn insert_header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<http::header::HeaderName>,
        K::Error: fmt::Debug,
        V: TryInto<http::header::HeaderValue>,
        V::Error: fmt::Debug,
    {
        let name: http::header::HeaderName = name.try_into().expect("invalid header name");
        let value: http::header::HeaderValue = value.try_into().expect("invalid header value");
        self.0.headers_mut().insert(name, value);
        self
    }

    /// Insert the provided header. Appending the value to any header that already exists with the same name.
    pub fn append_header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<http::header::HeaderName>,
        K::Error: fmt::Debug,
        V: TryInto<http::header::HeaderValue>,
        V::Error: fmt::Debug,
    {
        let name: http::header::HeaderName = name.try_into().expect("invalid header name");
        let value: http::header::HeaderValue = value.try_into().expect("invalid header value");
        self.0.headers_mut().append(name, value);
        self
    }

    /// Set the body of the header.
    pub fn body<B2>(self, body: B2) -> ResponseBuilder<B2> {
        ResponseBuilder(self.0.map(|_| body))
    }
}

/// respond with the provided status code and an empty body.
pub fn status_code(code: u16) -> ResponseBuilder<&'static str> {
    ResponseBuilder(
        http::Response::builder()
            .status(code)
            .body("")
            .expect("invalid status code"),
    )
}

/// respond with a body that is the json encoding of data.
///
/// The status code will be `200` and the content-type will be
/// `application/json`.
pub fn json_encoded<T>(data: T) -> ResponseBuilder<String>
where
    T: serde::Serialize,
{
    status_code(200)
        .append_header("Content-Type", "application/json")
        .body(serde_json::to_string(&data).expect("failed to serialize body"))
}

/// respond with a body that is the url encoding of data.
///
/// The status code will be `200` and the content-type will be
/// `application/x-www-form-urlencoded`.
pub fn url_encoded<T>(data: T) -> ResponseBuilder<String>
where
    T: serde::Serialize,
{
    status_code(200)
        .append_header("Content-Type", "application/x-www-form-urlencoded")
        .body(serde_urlencoded::to_string(&data).expect("failed to serialize body"))
}

impl<B> Responder for ResponseBuilder<B>
where
    B: Clone + Into<hyper::body::Bytes> + Send + fmt::Debug,
{
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::body::Bytes>> + Send + 'a>> {
        self.0.respond(req)
    }
}

/// Responder that delays the embedded response
pub struct Delay<R: Responder> {
    delay: Duration,
    and_then: R,
}

/// respond with the given responder after a delay
///
/// This is useful for testing request timeouts.
pub fn delay_and_then<R: Responder>(delay: Duration, and_then: R) -> Delay<R> {
    Delay { delay, and_then }
}

impl<R: Responder> Responder for Delay<R> {
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::body::Bytes>> + Send + 'a>> {
        let resp = self.and_then.respond(req);
        let delay = self.delay;

        Box::pin(async move {
            tokio::time::sleep(delay).await;
            resp.await
        })
    }
}

impl<B> Responder for http::Response<B>
where
    B: Clone + Into<hyper::body::Bytes> + Send + fmt::Debug,
{
    fn respond<'a>(
        &mut self,
        _req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::body::Bytes>> + Send + 'a>> {
        async fn _respond(
            resp: http::Response<hyper::body::Bytes>,
        ) -> http::Response<hyper::body::Bytes> {
            resp
        }
        let mut builder = http::Response::builder();
        builder = builder.status(self.status()).version(self.version());
        *builder.headers_mut().unwrap() = self.headers().clone();
        let resp = builder.body(self.body().clone().into()).unwrap();

        Box::pin(_respond(resp))
    }
}

/// Respond with the response returned from the provided function.
impl<F, B> Responder for F
where
    F: FnMut() -> B + Clone + Send + 'static,
    B: Responder,
{
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::body::Bytes>> + Send + 'a>> {
        let mut f = self.clone();
        Box::pin(async move { tokio::task::block_in_place(|| f()).respond(req).await })
    }
}

/// Cycle through the provided list of responders.
pub fn cycle(responders: Vec<Box<dyn Responder>>) -> impl Responder {
    if responders.is_empty() {
        panic!("empty vector provided to cycle");
    }
    Cycle { idx: 0, responders }
}
/// The `Cycle` responder returned by [cycle()](fn.cycle.html)
pub struct Cycle {
    idx: usize,
    responders: Vec<Box<dyn Responder>>,
}

impl Responder for Cycle {
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::body::Bytes>> + Send + 'a>> {
        let idx = self.idx;
        self.idx = (self.idx + 1) % self.responders.len();
        self.responders[idx].respond(req)
    }
}

#[cfg(test)]
mod tests {}
