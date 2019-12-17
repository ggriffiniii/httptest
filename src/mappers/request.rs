//! Mappers that extract information from HTTP requests.

use super::{Mapper, KV};

/// Extract the method from the HTTP request and pass it to the next mapper.
pub fn method<M>(inner: M) -> Method<M> {
    Method(inner)
}
/// The `Method` mapper returned by [method()](fn.method.html)
#[derive(Debug)]
pub struct Method<M>(M);
impl<M, B> Mapper<http::Request<B>> for Method<M>
where
    M: Mapper<str>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Request<B>) -> M::Out {
        self.0.map(input.method().as_str())
    }
}

/// Extract the path from the HTTP request and pass it to the next mapper.
pub fn path<M>(inner: M) -> Path<M> {
    Path(inner)
}
/// The `Path` mapper returned by [path()](fn.path.html)
#[derive(Debug)]
pub struct Path<M>(M);
impl<M, B> Mapper<http::Request<B>> for Path<M>
where
    M: Mapper<str>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Request<B>) -> M::Out {
        self.0.map(input.uri().path())
    }
}

/// Extract the query from the HTTP request and pass it to the next mapper.
pub fn query<M>(inner: M) -> Query<M> {
    Query(inner)
}
/// The `Query` mapper returned by [query()](fn.query.html)
#[derive(Debug)]
pub struct Query<M>(M);
impl<M, B> Mapper<http::Request<B>> for Query<M>
where
    M: Mapper<str>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Request<B>) -> M::Out {
        self.0.map(input.uri().query().unwrap_or(""))
    }
}

/// Extract the headers from the HTTP request and pass the sequence to the next
/// mapper.
pub fn headers<M>(inner: M) -> Headers<M> {
    Headers(inner)
}
/// The `Headers` mapper returned by [headers()](fn.headers.html)
#[derive(Debug)]
pub struct Headers<M>(M);
impl<M, B> Mapper<http::Request<B>> for Headers<M>
where
    M: Mapper<[KV<str, [u8]>]>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Request<B>) -> M::Out {
        let headers: Vec<KV<str, [u8]>> = input
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

/// Extract the body from the HTTP request and pass it to the next mapper.
pub fn body<M>(inner: M) -> Body<M> {
    Body(inner)
}
/// The `Body` mapper returned by [body()](fn.body.html)
#[derive(Debug)]
pub struct Body<M>(M);
impl<M, B> Mapper<http::Request<B>> for Body<M>
where
    B: ToOwned,
    M: Mapper<B::Owned>,
{
    type Out = M::Out;

    fn map(&mut self, input: &http::Request<B>) -> M::Out {
        self.0.map(&input.body().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappers::*;

    #[test]
    fn test_path() {
        let req = http::Request::get("https://example.com/foo")
            .body("")
            .unwrap();
        assert!(path("/foo").map(&req));

        let req = http::Request::get("https://example.com/foobar")
            .body("")
            .unwrap();
        assert!(path("/foobar").map(&req))
    }

    #[test]
    fn test_query() {
        let req = http::Request::get("https://example.com/path?foo=bar&baz=bat")
            .body("")
            .unwrap();
        assert!(query("foo=bar&baz=bat").map(&req));
        let req = http::Request::get("https://example.com/path?search=1")
            .body("")
            .unwrap();
        assert!(query("search=1").map(&req));
    }

    #[test]
    fn test_method() {
        let req = http::Request::get("https://example.com/foo")
            .body("")
            .unwrap();
        assert!(method("GET").map(&req));
        let req = http::Request::post("https://example.com/foobar")
            .body("")
            .unwrap();
        assert!(method("POST").map(&req));
    }

    #[test]
    fn test_headers() {
        let expected = vec![
            kv("host", &b"example.com"[..]),
            kv("content-length", b"101"),
        ];
        let mut req = http::Request::get("https://example.com/path?key%201=value%201&key2")
            .body("")
            .unwrap();
        req.headers_mut().extend(vec![
            (
                hyper::header::HOST,
                hyper::header::HeaderValue::from_static("example.com"),
            ),
            (
                hyper::header::CONTENT_LENGTH,
                hyper::header::HeaderValue::from_static("101"),
            ),
        ]);

        assert!(headers(eq(expected)).map(&req));
    }

    #[test]
    fn test_body() {
        let req = http::Request::get("https://example.com/foo")
            .body("my request body")
            .unwrap();
        assert!(body("my request body").map(&req));
    }
}
