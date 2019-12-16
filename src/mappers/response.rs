//! Mappers that extract information from HTTP responses.

use super::Mapper;
use crate::mappers::sequence::KV;

/// Extract the status code from the HTTP response and pass it to the next mapper.
pub fn status_code<M>(inner: M) -> StatusCode<M> {
    StatusCode(inner)
}
/// The `StatusCode` mapper returned by [status_code()](fn.status_code.html)
#[derive(Debug)]
pub struct StatusCode<M>(M);
impl<M, B> Mapper<http::Response<B>> for StatusCode<M>
where
    M: Mapper<u16>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Response<B>) -> M::Out {
        self.0.map(&input.status().as_u16())
    }
}

/// Extract the headers from the HTTP response and pass the sequence to the next
/// mapper.
pub fn headers<M>(inner: M) -> Headers<M> {
    Headers(inner)
}
/// The `Headers` mapper returned by [headers()](fn.headers.html)
#[derive(Debug)]
pub struct Headers<M>(M);
impl<M, B> Mapper<http::Response<B>> for Headers<M>
where
    M: Mapper<[KV<str, [u8]>]>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Response<B>) -> M::Out {
        let headers: Vec<_> = input
            .headers()
            .iter()
            .map(|(k, v)| KV {
                k: k.as_str().to_owned(),
                v: v.as_bytes().to_owned(),
            })
            .collect();
        self.0.map(&headers)
    }
}

/// Extract the body from the HTTP response and pass it to the next mapper.
pub fn body<M>(inner: M) -> Body<M> {
    Body(inner)
}
/// The `Body` mapper returned by [body()](fn.body.html)
#[derive(Debug)]
pub struct Body<M>(M);
impl<M, B> Mapper<http::Response<B>> for Body<M>
where
    M: Mapper<B>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Response<B>) -> M::Out {
        self.0.map(input.body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappers::*;

    #[test]
    fn test_status_code() {
        let resp = http::Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body("")
            .unwrap();
        assert!(status_code(eq(404)).map(&resp));

        let resp = http::Response::builder()
            .status(hyper::StatusCode::OK)
            .body("")
            .unwrap();
        assert!(status_code(eq(200)).map(&resp));
    }

    #[test]
    fn test_headers() {
        let expected: Vec<KV<str, [u8]>> = vec![
            KV {
                k: "host".to_owned(),
                v: Vec::from("example.com"),
            },
            KV {
                k: "content-length".to_owned(),
                v: Vec::from("101"),
            },
        ];
        let resp = http::Response::builder()
            .header("host", "example.com")
            .header("content-length", 101)
            .body("")
            .unwrap();

        assert!(headers(eq(expected)).map(&resp));
    }

    #[test]
    fn test_body() {
        let resp = http::Response::builder().body("my request body").unwrap();
        assert!(body(eq("my request body")).map(&resp));
    }
}
