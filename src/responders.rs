//! Responder implementations.
//!
//! Reponders determine how the server will respond.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

// import the cycle macro so that it's available if people glob import this module.
#[doc(inline)]
pub use crate::cycle;

/// Respond with an HTTP response.
pub trait Responder: Send + fmt::Debug {
    /// Return a future that outputs an HTTP response.
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = hyper::Response<hyper::Body>> + Send>>;
}

/// respond with the provided status code.
pub fn status_code(code: u16) -> impl Responder {
    StatusCode(code)
}
/// The `StatusCode` responder returned by [status_code()](fn.status_code.html)
#[derive(Debug)]
pub struct StatusCode(u16);
impl Responder for StatusCode {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = hyper::Response<hyper::Body>> + Send>> {
        async fn _respond(status_code: u16) -> hyper::Response<hyper::Body> {
            hyper::Response::builder()
                .status(status_code)
                .body(hyper::Body::empty())
                .unwrap()
        }
        Box::pin(_respond(self.0))
    }
}

/// respond with a body that is the json encoding of data.
///
/// The status code will be `200` and the content-type will be
/// `application/json`.
pub fn json_encoded<T>(data: T) -> impl Responder
where
    T: serde::Serialize,
{
    JsonEncoded(serde_json::to_string(&data).unwrap())
}
/// The `JsonEncoded` responder returned by [json_encoded()](fn.json_encoded.html)
#[derive(Debug)]
pub struct JsonEncoded(String);
impl Responder for JsonEncoded {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = hyper::Response<hyper::Body>> + Send>> {
        async fn _respond(body: String) -> hyper::Response<hyper::Body> {
            hyper::Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(body.into())
                .unwrap()
        }
        Box::pin(_respond(self.0.clone()))
    }
}

/// respond with a body that is the url encoding of data.
///
/// The status code will be `200` and the content-type will be
/// `application/x-www-form-urlencoded`.
pub fn url_encoded<T>(data: T) -> impl Responder
where
    T: serde::Serialize,
{
    UrlEncoded(serde_urlencoded::to_string(&data).unwrap())
}
/// The `UrlEncoded` responder returned by [url_encoded()](fn.url_encoded.html)
#[derive(Debug)]
pub struct UrlEncoded(String);
impl Responder for UrlEncoded {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = hyper::Response<hyper::Body>> + Send>> {
        async fn _respond(body: String) -> hyper::Response<hyper::Body> {
            hyper::Response::builder()
                .status(200)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(body.into())
                .unwrap()
        }
        Box::pin(_respond(self.0.clone()))
    }
}

impl<B> Responder for hyper::Response<B>
where
    B: Clone + Into<hyper::Body> + Send + fmt::Debug,
{
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = hyper::Response<hyper::Body>> + Send>> {
        async fn _respond(resp: hyper::Response<hyper::Body>) -> hyper::Response<hyper::Body> {
            resp
        }
        // Turn &hyper::Response<Vec<u8>> into a hyper::Response<hyper::Body>
        let mut builder = hyper::Response::builder();
        builder = builder
            .status(self.status().clone())
            .version(self.version().clone());
        *builder.headers_mut().unwrap() = self.headers().clone();
        let resp = builder.body(self.body().clone().into()).unwrap();

        Box::pin(_respond(resp))
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
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = hyper::Response<hyper::Body>> + Send>> {
        let response = self.responders[self.idx].respond();
        self.idx = (self.idx + 1) % self.responders.len();
        response
    }
}

#[cfg(test)]
mod tests {}
