//! Mappers that extract information from HTTP responses.

use super::Mapper;

/// Extract the status code from the HTTP response and pass it to the next mapper.
pub fn status_code<C>(inner: C) -> StatusCode<C> {
    StatusCode(inner)
}
/// The `StatusCode` mapper returned by [status_code()](fn.status_code.html)
#[derive(Debug)]
pub struct StatusCode<C>(C);
impl<C, B> Mapper<hyper::Response<B>> for StatusCode<C>
where
    C: Mapper<u16>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Response<B>) -> C::Out {
        self.0.map(&input.status().as_u16())
    }
}

/// Extract the headers from the HTTP response and pass the sequence to the next
/// mapper.
pub fn headers<C>(inner: C) -> Headers<C> {
    Headers(inner)
}
/// The `Headers` mapper returned by [headers()](fn.headers.html)
#[derive(Debug)]
pub struct Headers<C>(C);
impl<C, B> Mapper<hyper::Response<B>> for Headers<C>
where
    C: Mapper<[(Vec<u8>, Vec<u8>)]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Response<B>) -> C::Out {
        let headers: Vec<(Vec<u8>, Vec<u8>)> = input
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().into(), v.as_bytes().into()))
            .collect();
        self.0.map(&headers)
    }
}

/// Extract the body from the HTTP response and pass it to the next mapper.
pub fn body<C>(inner: C) -> Body<C> {
    Body(inner)
}
/// The `Body` mapper returned by [body()](fn.body.html)
#[derive(Debug)]
pub struct Body<C>(C);
impl<C, B> Mapper<hyper::Response<B>> for Body<C>
where
    C: Mapper<B>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Response<B>) -> C::Out {
        self.0.map(input.body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappers::*;

    #[test]
    fn test_status_code() {
        let resp = hyper::Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body("")
            .unwrap();
        assert!(status_code(eq(404)).map(&resp));

        let resp = hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .body("")
            .unwrap();
        assert!(status_code(eq(200)).map(&resp));
    }

    #[test]
    fn test_headers() {
        let expected = vec![
            (Vec::from("host"), Vec::from("example.com")),
            (Vec::from("content-length"), Vec::from("101")),
        ];
        let resp = hyper::Response::builder()
            .header("host", "example.com")
            .header("content-length", 101)
            .body("")
            .unwrap();

        assert!(headers(eq(expected)).map(&resp));
    }

    #[test]
    fn test_body() {
        let resp = hyper::Response::builder().body("my request body").unwrap();
        assert!(body(eq("my request body")).map(&resp));
    }
}
