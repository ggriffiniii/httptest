use super::Mapper;

pub fn method<C>(inner: C) -> Method<C>
where
    C: Mapper<str>,
{
    Method(inner)
}
#[derive(Debug)]
pub struct Method<C>(C);
impl<C, B> Mapper<hyper::Request<B>> for Method<C>
where
    C: Mapper<str>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Request<B>) -> C::Out {
        self.0.map(input.method().as_str())
    }
}

pub fn path<C>(inner: C) -> Path<C>
where
    C: Mapper<str>,
{
    Path(inner)
}
#[derive(Debug)]
pub struct Path<C>(C);
impl<C, B> Mapper<hyper::Request<B>> for Path<C>
where
    C: Mapper<str>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Request<B>) -> C::Out {
        self.0.map(input.uri().path())
    }
}

pub fn query<C>(inner: C) -> Query<C>
where
    C: Mapper<str>,
{
    Query(inner)
}
#[derive(Debug)]
pub struct Query<C>(C);
impl<C, B> Mapper<hyper::Request<B>> for Query<C>
where
    C: Mapper<str>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Request<B>) -> C::Out {
        self.0.map(input.uri().query().unwrap_or(""))
    }
}

pub fn headers<C>(inner: C) -> Headers<C>
where
    C: Mapper<[(Vec<u8>, Vec<u8>)]>,
{
    Headers(inner)
}
#[derive(Debug)]
pub struct Headers<C>(C);
impl<C, B> Mapper<hyper::Request<B>> for Headers<C>
where
    C: Mapper<[(Vec<u8>, Vec<u8>)]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Request<B>) -> C::Out {
        let headers: Vec<(Vec<u8>, Vec<u8>)> = input
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().into(), v.as_bytes().into()))
            .collect();
        self.0.map(&headers)
    }
}

pub fn body<C>(inner: C) -> Body<C> {
    Body(inner)
}
#[derive(Debug)]
pub struct Body<C>(C);
impl<C, B> Mapper<hyper::Request<B>> for Body<C>
where
    B: ToOwned,
    C: Mapper<B::Owned>,
{
    type Out = C::Out;

    fn map(&mut self, input: &hyper::Request<B>) -> C::Out {
        self.0.map(&input.body().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappers::*;

    #[test]
    fn test_path() {
        let req = hyper::Request::get("https://example.com/foo")
            .body("")
            .unwrap();
        assert!(path(eq("/foo")).map(&req));

        let req = hyper::Request::get("https://example.com/foobar")
            .body("")
            .unwrap();
        assert!(path(eq("/foobar")).map(&req))
    }

    #[test]
    fn test_query() {
        let req = hyper::Request::get("https://example.com/path?foo=bar&baz=bat")
            .body("")
            .unwrap();
        assert!(query(eq("foo=bar&baz=bat")).map(&req));
        let req = hyper::Request::get("https://example.com/path?search=1")
            .body("")
            .unwrap();
        assert!(query(eq("search=1")).map(&req));
    }

    #[test]
    fn test_method() {
        let req = hyper::Request::get("https://example.com/foo")
            .body("")
            .unwrap();
        assert!(method(eq("GET")).map(&req));
        let req = hyper::Request::post("https://example.com/foobar")
            .body("")
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
        let req = hyper::Request::get("https://example.com/foo")
            .body("my request body")
            .unwrap();
        assert!(body(eq("my request body")).map(&req));
    }
}
