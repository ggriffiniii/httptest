//! Responder implementations.
//!
//! Reponders determine how the server will respond.

use std::convert::TryInto;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

// import the cycle macro so that it's available if people glob import this module.
#[doc(inline)]
pub use crate::cycle;

/// Respond with an HTTP response.
pub trait Responder: Send + fmt::Debug {
    /// Return a future that outputs an HTTP response.
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'a>>;
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
    B: Clone + Into<hyper::Body> + Send + fmt::Debug,
{
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
        self.0.respond(req)
    }
}

impl<B> Responder for http::Response<B>
where
    B: Clone + Into<hyper::Body> + Send + fmt::Debug,
{
    fn respond<'a>(
        &mut self,
        _req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
        async fn _respond(resp: http::Response<hyper::Body>) -> http::Response<hyper::Body> {
            resp
        }
        let mut builder = http::Response::builder();
        builder = builder
            .status(self.status().clone())
            .version(self.version().clone());
        *builder.headers_mut().unwrap() = self.headers().clone();
        let resp = builder.body(self.body().clone().into()).unwrap();

        Box::pin(_respond(resp))
    }
}

/// Respond with the response returned from the provided function.
pub fn from_fn<F, B>(f: F) -> FnResponder<F, B>
where
    F: FnMut(&http::Request<bytes::Bytes>) -> B + Clone + Send + 'static,
    B: Responder,
{
    FnResponder(f, std::marker::PhantomData)
}
/// The `FnResponder` responder returned by [from_fn()](fn.from_fn.html)
pub struct FnResponder<F, B>(F, std::marker::PhantomData<B>);

impl<F, B> fmt::Debug for FnResponder<F, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FnResponder").finish()
    }
}

impl<F, B> Responder for FnResponder<F, B>
where
    F: FnMut(&http::Request<bytes::Bytes>) -> B + Clone + Send + 'static,
    B: Responder,
{
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
        let mut f = self.0.clone();
        Box::pin(async move { tokio::task::block_in_place(|| f(req)).respond(req).await })
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
#[derive(Debug)]
pub struct Cycle {
    idx: usize,
    responders: Vec<Box<dyn Responder>>,
}
impl Responder for Cycle {
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
        let idx = self.idx;
        self.idx = (self.idx + 1) % self.responders.len();
        self.responders[idx].respond(req)
    }
}

#[cfg(test)]
mod tests {}
