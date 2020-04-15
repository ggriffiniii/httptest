//! Matchers that extract information from HTTP requests.

use super::{matcher_name, ExecutionContext, Matcher, KV};
use std::fmt;

/// Extract the method from the HTTP request and pass it to the next mapper.
pub fn method<M>(inner: M) -> Method<M> {
    Method(inner)
}
/// The `Method` mapper returned by [method()](fn.method.html)
#[derive(Debug)]
pub struct Method<M>(M);
impl<M, B> Matcher<http::Request<B>> for Method<M>
where
    M: Matcher<str>,
{
    fn matches(&mut self, input: &http::Request<B>, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, input.method().as_str())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Method")
            .field(&matcher_name(&self.0))
            .finish()
    }
}

/// Extract the path from the HTTP request and pass it to the next mapper.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a path of `/foo`.
/// request::path("/foo");
///
/// // A request matcher that matches a path of `/foo` or `/bar`.
/// request::path(matches("^/(foo|bar)$"));
/// ```
pub fn path<M>(inner: M) -> Path<M> {
    Path(inner)
}
/// The `Path` mapper returned by [path()](fn.path.html)
#[derive(Debug)]
pub struct Path<M>(M);
impl<M, B> Matcher<http::Request<B>> for Path<M>
where
    M: Matcher<str>,
{
    fn matches(&mut self, input: &http::Request<B>, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, input.uri().path())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Path").field(&matcher_name(&self.0)).finish()
    }
}

/// Extract the query from the HTTP request and pass it to the next mapper.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request with a query parameter `foobar=value`.
/// request::query(url_decoded(contains(("foobar", "value"))));
/// ```
pub fn query<M>(inner: M) -> Query<M> {
    Query(inner)
}
/// The `Query` mapper returned by [query()](fn.query.html)
#[derive(Debug)]
pub struct Query<M>(M);
impl<M, B> Matcher<http::Request<B>> for Query<M>
where
    M: Matcher<str>,
{
    fn matches(&mut self, input: &http::Request<B>, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, input.uri().query().unwrap_or(""))
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Query")
            .field(&matcher_name(&self.0))
            .finish()
    }
}

/// Extract the headers from the HTTP request and pass the sequence to the next
/// mapper.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request with the header `x-foobar: value`.
/// request::headers(contains(("x-foobar", "value")));
///
/// // A request matcher that matches a request with the header `x-foobar` with any value.
/// request::headers(contains(key("x-foobar")));
/// ```
pub fn headers<M>(inner: M) -> Headers<M> {
    Headers(inner)
}
/// The `Headers` mapper returned by [headers()](fn.headers.html)
#[derive(Debug)]
pub struct Headers<M>(M);
impl<M, B> Matcher<http::Request<B>> for Headers<M>
where
    M: Matcher<[KV<str, bstr::BStr>]>,
{
    fn matches(&mut self, input: &http::Request<B>, ctx: &mut ExecutionContext) -> bool {
        use bstr::{BStr, ByteSlice};
        let headers: Vec<KV<str, BStr>> = input
            .headers()
            .iter()
            .map(|(k, v)| KV {
                k: k.as_str().to_owned(),
                v: v.as_bytes().as_bstr().to_owned(),
            })
            .collect();
        ctx.chain(&mut self.0, &headers)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Headers")
            .field(&matcher_name(&self.0))
            .finish()
    }
}

/// Extract the body from the HTTP request and pass it to the next mapper.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a body of `foobar`.
/// request::body("foobar");
///
/// // A request matcher that matches a json encoded body of `{"foo": 1}`
/// request::body(json_decoded(eq(serde_json::json!({
///     "foo": 1,
/// }))));
/// ```
pub fn body<M>(inner: M) -> Body<M> {
    Body(inner)
}
/// The `Body` mapper returned by [body()](fn.body.html)
#[derive(Debug)]
pub struct Body<M>(M);

impl<M, B> Matcher<http::Request<B>> for Body<M>
where
    B: AsRef<[u8]>,
    M: Matcher<bstr::BStr>,
{
    fn matches(&mut self, input: &http::Request<B>, ctx: &mut ExecutionContext) -> bool {
        use bstr::ByteSlice;
        ctx.chain(&mut self.0, input.body().as_ref().as_bstr())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Body").field(&matcher_name(&self.0)).finish()
    }
}

/// A convenience matcher for both method and path. Extracts a bolean true if the method and path both match.
///
/// `method_path(a, b) == all_of![method(a), path(b)]`
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a `GET` request to `/foo`.
/// request::method_path("GET", "/foo");
/// ```
pub fn method_path<M, P>(method: M, path: P) -> MethodPath<M, P> {
    MethodPath { method, path }
}
/// The `MethodPath` mapper returned by [method_path()](fn.method_path.html)
#[derive(Debug)]
pub struct MethodPath<M, P> {
    method: M,
    path: P,
}
impl<M, P, B> Matcher<http::Request<B>> for MethodPath<M, P>
where
    M: Matcher<str>,
    P: Matcher<str>,
{
    fn matches(&mut self, input: &http::Request<B>, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.method, input.method().as_str())
            && ctx.chain(&mut self.path, input.uri().path())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MethodPath")
            .field("method", &matcher_name(&self.method))
            .field("path", &matcher_name(&self.path))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matchers::*;

    fn eval<M, I>(matcher: &mut M, input: &I) -> bool
    where
        M: Matcher<I> + ?Sized,
        I: fmt::Debug + ?Sized,
    {
        ExecutionContext::evaluate(matcher, input)
    }

    #[test]
    fn test_path() {
        let req = http::Request::get("https://example.com/foo")
            .body("")
            .unwrap();
        assert!(eval(&mut path("/foo"), &req));

        let req = http::Request::get("https://example.com/foobar")
            .body("")
            .unwrap();

        assert!(eval(&mut path("/foobar"), &req));
    }

    #[test]
    fn test_query() {
        let req = http::Request::get("https://example.com/path?foo=bar&baz=bat")
            .body("")
            .unwrap();
        assert!(eval(&mut query("foo=bar&baz=bat"), &req));
        let req = http::Request::get("https://example.com/path?search=1")
            .body("")
            .unwrap();
        assert!(eval(&mut query("search=1"), &req));
    }

    #[test]
    fn test_method() {
        let req = http::Request::get("https://example.com/foo")
            .body("")
            .unwrap();
        assert!(eval(&mut method("GET"), &req));
        let req = http::Request::post("https://example.com/foobar")
            .body("")
            .unwrap();
        assert!(eval(&mut method("POST"), &req));
    }

    #[test]
    fn test_headers() {
        use bstr::{ByteSlice, B};
        let expected = vec![
            KV::new("host", B("example.com").as_bstr()),
            KV::new("content-length", B("101").as_bstr()),
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

        assert!(eval(&mut headers(eq(expected)), &req));
    }

    #[test]
    fn test_body() {
        let req = http::Request::get("https://example.com/foo")
            .body("my request body")
            .unwrap();
        assert!(eval(&mut body("my request body"), &req));
    }

    #[test]
    fn test_method_path() {
        let req = http::Request::get("https://example.com/foo")
            .body("")
            .unwrap();
        assert!(eval(&mut method_path("GET", "/foo"), &req));
        assert!(!eval(&mut method_path("POST", "/foo"), &req));
        assert!(!eval(&mut method_path("GET", "/"), &req));

        let req = http::Request::post("https://example.com/foobar")
            .body("")
            .unwrap();
        assert!(eval(&mut method_path("POST", "/foobar"), &req));
        assert!(!eval(&mut method_path("GET", "/foobar"), &req));
        assert!(!eval(&mut method_path("POST", "/"), &req));
    }
}
