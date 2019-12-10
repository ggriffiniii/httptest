use std::fmt;
use std::future::Future;
use std::pin::Pin;

pub use crate::cycle;

pub trait Responder: Send + fmt::Debug {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send>>;
}

pub fn status_code(code: u16) -> impl Responder {
    StatusCode(code)
}
#[derive(Debug)]
pub struct StatusCode(u16);
impl Responder for StatusCode {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send>> {
        async fn _respond(status_code: u16) -> http::Response<hyper::Body> {
            hyper::Response::builder()
                .status(status_code)
                .body(hyper::Body::empty())
                .unwrap()
        }
        Box::pin(_respond(self.0))
    }
}

pub fn json_encoded<T>(data: T) -> impl Responder
where
    T: serde::Serialize,
{
    JsonEncoded(serde_json::to_string(&data).unwrap())
}
#[derive(Debug)]
pub struct JsonEncoded(String);
impl Responder for JsonEncoded {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send>> {
        async fn _respond(body: String) -> http::Response<hyper::Body> {
            hyper::Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(body.into())
                .unwrap()
        }
        Box::pin(_respond(self.0.clone()))
    }
}

pub fn url_encoded<T>(data: T) -> impl Responder
where
    T: serde::Serialize,
{
    UrlEncoded(serde_urlencoded::to_string(&data).unwrap())
}
#[derive(Debug)]
pub struct UrlEncoded(String);
impl Responder for UrlEncoded {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send>> {
        async fn _respond(body: String) -> http::Response<hyper::Body> {
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
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send>> {
        async fn _respond(resp: http::Response<hyper::Body>) -> http::Response<hyper::Body> {
            resp
        }
        // Turn &hyper::Response<Vec<u8>> into a hyper::Response<hyper::Body>
        let mut builder = hyper::Response::builder();
        builder
            .status(self.status().clone())
            .version(self.version().clone());
        *builder.headers_mut().unwrap() = self.headers().clone();
        let resp = builder.body(self.body().clone().into()).unwrap();

        Box::pin(_respond(resp))
    }
}

pub fn cycle(responders: Vec<Box<dyn Responder>>) -> impl Responder {
    if responders.is_empty() {
        panic!("empty vector provided to cycle");
    }
    Cycle { idx: 0, responders }
}
#[derive(Debug)]
pub struct Cycle {
    idx: usize,
    responders: Vec<Box<dyn Responder>>,
}
impl Responder for Cycle {
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send>> {
        let response = self.responders[self.idx].respond();
        self.idx = (self.idx + 1) % self.responders.len();
        response
    }
}

#[cfg(test)]
mod tests {}
