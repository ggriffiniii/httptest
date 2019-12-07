use super::Mapper;
use crate::FullRequest;

pub fn method<C>(inner: C) -> impl Mapper<FullRequest, Out = C::Out>
where
    C: Mapper<str>,
{
    Method(inner)
}
#[derive(Debug)]
pub struct Method<C>(C);
impl<C> Mapper<FullRequest> for Method<C>
where
    C: Mapper<str>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullRequest) -> C::Out {
        self.0.map(input.method().as_str())
    }
}

pub fn path<C>(inner: C) -> impl Mapper<FullRequest, Out = C::Out>
where
    C: Mapper<str>,
{
    Path(inner)
}
#[derive(Debug)]
pub struct Path<C>(C);
impl<C> Mapper<FullRequest> for Path<C>
where
    C: Mapper<str>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullRequest) -> C::Out {
        self.0.map(input.uri().path())
    }
}

pub fn query<C>(inner: C) -> impl Mapper<FullRequest, Out = C::Out>
where
    C: Mapper<str>,
{
    Query(inner)
}
#[derive(Debug)]
pub struct Query<C>(C);
impl<C> Mapper<FullRequest> for Query<C>
where
    C: Mapper<str>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullRequest) -> C::Out {
        self.0.map(input.uri().query().unwrap_or(""))
    }
}

pub fn headers<C>(inner: C) -> impl Mapper<FullRequest, Out = C::Out>
where
    C: Mapper<[(Vec<u8>, Vec<u8>)]>,
{
    Headers(inner)
}
#[derive(Debug)]
pub struct Headers<C>(C);
impl<C> Mapper<FullRequest> for Headers<C>
where
    C: Mapper<[(Vec<u8>, Vec<u8>)]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullRequest) -> C::Out {
        let headers: Vec<(Vec<u8>, Vec<u8>)> = input
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().into(), v.as_bytes().into()))
            .collect();
        self.0.map(&headers)
    }
}

pub fn body<C>(inner: C) -> impl Mapper<FullRequest, Out = C::Out>
where
    C: Mapper<[u8]>,
{
    Body(inner)
}
#[derive(Debug)]
pub struct Body<C>(C);
impl<C> Mapper<FullRequest> for Body<C>
where
    C: Mapper<[u8]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullRequest) -> C::Out {
        self.0.map(input.body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappers::*;

    #[test]
    fn test_path() {
        let req = hyper::Request::get("https://example.com/foo")
            .body(Vec::new())
            .unwrap();
        assert!(path(eq("/foo")).map(&req));

        let req = hyper::Request::get("https://example.com/foobar")
            .body(Vec::new())
            .unwrap();
        assert!(path(eq("/foobar")).map(&req))
    }

    #[test]
    fn test_query() {
        let req = hyper::Request::get("https://example.com/path?foo=bar&baz=bat")
            .body(Vec::new())
            .unwrap();
        assert!(query(eq("foo=bar&baz=bat")).map(&req));
        let req = hyper::Request::get("https://example.com/path?search=1")
            .body(Vec::new())
            .unwrap();
        assert!(query(eq("search=1")).map(&req));
    }

    #[test]
    fn test_method() {
        let req = hyper::Request::get("https://example.com/foo")
            .body(Vec::new())
            .unwrap();
        assert!(method(eq("GET")).map(&req));
        let req = hyper::Request::post("https://example.com/foobar")
            .body(Vec::new())
            .unwrap();
        assert!(method(eq("POST")).map(&req));
    }

    #[test]
    fn test_headers() {
        let expected = vec![
            (Vec::from("host"), Vec::from("example.com")),
            (Vec::from("content-length"), Vec::from("101")),
        ];
        let mut req = hyper::Request::get("https://example.com/path?key%201=value%201&key2")
            .body(Vec::new())
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
        use bstr::{ByteVec, B};
        let req = hyper::Request::get("https://example.com/foo")
            .body(Vec::from_slice("my request body"))
            .unwrap();
        assert!(body(eq(B("my request body"))).map(&req));
    }
}
