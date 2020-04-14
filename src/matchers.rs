//! Matcher implementations.
//!
//! This module contains matchers for composing a set of operations. The result
//! of the composition usually results in a boolean.

use std::borrow::Borrow;
use std::fmt;
use std::marker::PhantomData;

// import the any_of and all_of macros from crate root so they are accessible if
// people glob import this module.
#[doc(inline)]
pub use crate::all_of;
#[doc(inline)]
pub use crate::any_of;

pub mod request;

/// An ExecutionContext tracks how Matchers are chained together. There is a
/// single public method called chain that when used to chain input from one
/// matcher to another will allow tracking the flow of data across composable
/// matchers.
pub struct ExecutionContext {
    // Users outside this crate should not need to construct an ExecutionContext.
    stack_depth: usize,
}

impl ExecutionContext {
    /// Evaluate the given matcher with the provided input.
    pub fn evaluate<M, I>(matcher: &mut M, input: &I) -> bool
    where
        M: Matcher<I> + ?Sized,
        I: fmt::Debug + ?Sized,
    {
        let mut ctx = ExecutionContext { stack_depth: 0 };
        log::debug!(
            "Matching {:?} with input: {:?}",
            matcher_name(matcher),
            input
        );
        let x = matcher.matches(input, &mut ctx);
        log::debug!(
            "┗━ {}",
            if x {
                "✅ matches"
            } else {
                "❌ does not match"
            }
        );
        x
    }

    /// Invoke the provided matcher with the provided input. This is equivalent
    /// to invoking `matcher.matches(input)`, but allows tracking the execution
    /// flow to provide better diagnostics about why a request did or did not
    /// match a composed set of matchers.
    pub fn chain<M, I>(&mut self, matcher: &mut M, input: &I) -> bool
    where
        M: Matcher<I> + ?Sized,
        I: fmt::Debug + ?Sized,
    {
        self.stack_depth += 1;
        log::debug!(
            "{}Matching {:?} with input: {:?}",
            VerticalLines {
                num_lines: self.stack_depth
            },
            matcher_name(matcher),
            input
        );
        let x = matcher.matches(input, self);
        log::debug!(
            "{}┗━ {}",
            VerticalLines {
                num_lines: self.stack_depth
            },
            x
        );
        self.stack_depth -= 1;
        x
    }
}

struct VerticalLines {
    num_lines: usize,
}

impl fmt::Display for VerticalLines {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for _ in 0..self.num_lines {
            write!(f, "┃ ")?;
        }
        Ok(())
    }
}

/// The core trait. Defines how an input value should be turned into an output
/// value. This allows for a flexible pattern of composition where two or more
/// matchers are chained together to form a readable and flexible manipulation.
pub trait Matcher<IN>: Send
where
    IN: ?Sized,
{
    /// Map an input to output.
    fn matches(&mut self, input: &IN, ctx: &mut ExecutionContext) -> bool;

    /// formatted name of the mapper. This is used for debugging purposes and
    /// should typically look like a fmt::Debug representation.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result;
}

/// convenience function to print the Matcher::fmt representation of a mapper.
/// Returns an object with a fmt::Debug matching the Matcher::fmt.
pub(crate) fn matcher_name<M, IN>(mapper: &M) -> MatcherName<'_, M, IN>
where
    M: ?Sized,
    IN: ?Sized,
{
    MatcherName(mapper, PhantomData)
}
pub(crate) struct MatcherName<'a, M, IN>(&'a M, PhantomData<&'a IN>)
where
    M: ?Sized,
    IN: ?Sized;
impl<'a, M, IN> fmt::Debug for MatcherName<'a, M, IN>
where
    M: Matcher<IN> + ?Sized,
    IN: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Always true.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a query parameter `foobar` with any value.
/// request::query(url_decoded(contains(("foobar", any()))));
/// ```
pub fn any() -> Any {
    Any
}
/// The `Any` mapper returned by [any()](fn.any.html)
#[derive(Debug)]
pub struct Any;
impl<IN> Matcher<IN> for Any
where
    IN: ?Sized,
{
    fn matches(&mut self, _input: &IN, _ctx: &mut ExecutionContext) -> bool {
        true
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// true if the input is equal to value.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// request::body(json_decoded(eq(serde_json::json!({
///     "foo": 1,
/// }))));
/// ```
pub fn eq<T>(value: T) -> Eq<T> {
    Eq(value)
}
/// The `Eq` mapper returned by [eq()](fn.eq.html)
pub struct Eq<T>(T)
where
    T: ?Sized;
impl<IN, T> Matcher<IN> for Eq<T>
where
    T: Borrow<IN> + fmt::Debug + Send + ?Sized,
    IN: PartialEq + ?Sized,
{
    fn matches(&mut self, input: &IN, _ctx: &mut ExecutionContext) -> bool {
        self.0.borrow() == input
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}
impl<T> fmt::Debug for Eq<T>
where
    T: fmt::Debug + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Eq({:?})", &self.0)
    }
}

/// A &str is an implicit Eq mapper.
impl<IN> Matcher<IN> for &str
where
    IN: AsRef<[u8]> + ?Sized,
{
    fn matches(&mut self, input: &IN, _ctx: &mut ExecutionContext) -> bool {
        self.as_bytes() == input.as_ref()
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// A String is an implicit Eq mapper.
impl<IN> Matcher<IN> for String
where
    IN: AsRef<[u8]> + ?Sized,
{
    fn matches(&mut self, input: &IN, _ctx: &mut ExecutionContext) -> bool {
        self.as_bytes() == input.as_ref()
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// A &[u8] is an implicit Eq mapper.
impl<IN> Matcher<IN> for &[u8]
where
    IN: AsRef<[u8]> + ?Sized,
{
    fn matches(&mut self, input: &IN, _ctx: &mut ExecutionContext) -> bool {
        *self == input.as_ref()
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// Create a regex.
///
/// This trait may panic if the regex failed to build.
pub trait IntoRegex {
    /// turn self into a regex.
    fn into_regex(self) -> regex::bytes::Regex;
}
impl IntoRegex for &str {
    fn into_regex(self) -> regex::bytes::Regex {
        regex::bytes::Regex::new(self).expect("failed to create regex")
    }
}
impl IntoRegex for String {
    fn into_regex(self) -> regex::bytes::Regex {
        regex::bytes::Regex::new(&self).expect("failed to create regex")
    }
}
impl IntoRegex for &mut regex::bytes::RegexBuilder {
    fn into_regex(self) -> regex::bytes::Regex {
        self.build().expect("failed to create regex")
    }
}
impl IntoRegex for regex::bytes::Regex {
    fn into_regex(self) -> regex::bytes::Regex {
        self
    }
}

/// true if the input matches the regex provided.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request to path "/test/foo" or "/test/bar".
/// request::path(matches("^/test/(foo|bar)$"));
/// ```
pub fn matches(value: impl IntoRegex) -> Matches {
    //let regex = regex::bytes::Regex::new(value).expect("failed to create regex");
    Matches(value.into_regex())
}
/// The `Matches` mapper returned by [matches()](fn.matches.html)
#[derive(Debug)]
pub struct Matches(regex::bytes::Regex);
impl<IN> Matcher<IN> for Matches
where
    IN: AsRef<[u8]> + ?Sized,
{
    fn matches(&mut self, input: &IN, _ctx: &mut ExecutionContext) -> bool {
        self.0.is_match(input.as_ref())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// invert the result of the inner mapper.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches if there is no `foobar` query parameter.
/// request::query(url_decoded(not(contains(key("foobar")))));
/// ```
pub fn not<M>(inner: M) -> Not<M> {
    Not(inner)
}
/// The `Not` mapper returned by [not()](fn.not.html)
pub struct Not<M>(M);
impl<M, IN> Matcher<IN> for Not<M>
where
    M: Matcher<IN>,
    IN: fmt::Debug + ?Sized,
{
    fn matches(&mut self, input: &IN, ctx: &mut ExecutionContext) -> bool {
        !ctx.chain(&mut self.0, input)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Not").field(&matcher_name(&self.0)).finish()
    }
}

/// true if all the provided matchers return true. See the `all_of!` macro for
/// convenient usage.
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a POST with a path that matches the regex 'foo.*'.
/// let mut m = all_of![
///     request::method("POST"),
///     request::path(matches("foo.*")),
/// ];
///
/// # // Allow type inference to determine the request type.
/// # ExecutionContext::evaluate(&mut m, &http::Request::get("/").body("").unwrap());
/// ```
pub fn all_of<IN>(inner: Vec<Box<dyn Matcher<IN>>>) -> AllOf<IN>
where
    IN: ?Sized,
{
    AllOf(inner)
}

/// The `AllOf` mapper returned by [all_of()](fn.all_of.html)
pub struct AllOf<IN>(Vec<Box<dyn Matcher<IN>>>)
where
    IN: ?Sized;
impl<IN> Matcher<IN> for AllOf<IN>
where
    IN: fmt::Debug + ?Sized,
{
    fn matches(&mut self, input: &IN, ctx: &mut ExecutionContext) -> bool {
        self.0
            .iter_mut()
            .all(|mapper| ctx.chain(mapper.as_mut(), input))
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

impl<IN> fmt::Debug for AllOf<IN>
where
    IN: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("AllOf")?;
        f.debug_list()
            .entries(self.0.iter().map(|x| matcher_name(&**x)))
            .finish()
    }
}

/// true if any of the provided matchers returns true. See the `any_of!` macro
/// for convenient usage.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request to path "/foo"
/// // or matches the reqex '^/test/(foo|bar)$'.
/// let mut m = any_of![
///     request::path("/foo"),
///     request::path(matches("^/test/(foo|bar)$")),
/// ];
///
/// # // Allow type inference to determine the request type.
/// # ExecutionContext::evaluate(&mut m, &http::Request::get("/").body("").unwrap());
/// ```
pub fn any_of<IN>(inner: Vec<Box<dyn Matcher<IN>>>) -> AnyOf<IN>
where
    IN: ?Sized,
{
    AnyOf(inner)
}
/// The `AnyOf` mapper returned by [any_of()](fn.any_of.html)
pub struct AnyOf<IN>(Vec<Box<dyn Matcher<IN>>>)
where
    IN: ?Sized;
impl<IN> Matcher<IN> for AnyOf<IN>
where
    IN: fmt::Debug + ?Sized,
{
    fn matches(&mut self, input: &IN, ctx: &mut ExecutionContext) -> bool {
        self.0
            .iter_mut()
            .any(|mapper| ctx.chain(mapper.as_mut(), input))
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

impl<IN> fmt::Debug for AnyOf<IN>
where
    IN: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("AnyOf")?;
        f.debug_list()
            .entries(self.0.iter().map(|x| matcher_name(&**x)))
            .finish()
    }
}

/// A key-value pair.
#[derive(Debug, PartialEq, PartialOrd)]
pub struct KV<K, V>
where
    Self: Sized,
    K: ToOwned + ?Sized,
    V: ToOwned + ?Sized,
{
    /// The key
    pub k: K::Owned,
    /// The value
    pub v: V::Owned,
}

impl<K, V> KV<K, V>
where
    K: ToOwned + ?Sized,
    V: ToOwned + ?Sized,
{
    /// Create a new KV. This will clone the provided k and v.
    pub fn new(k: &K, v: &V) -> Self {
        KV {
            k: k.to_owned(),
            v: v.to_owned(),
        }
    }
}

/// url decode the input and pass the resulting slice of key-value pairs to the next mapper.
///
/// # Example
///
/// ```rust
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request with a query parameter `foobar=value`.
/// request::query(url_decoded(contains(("foobar", "value"))));
///
/// // A request matcher that matches a request with a form-urlencoded parameter `foobar=value`.
/// request::body(url_decoded(contains(("foobar", "value"))));
/// ```
pub fn url_decoded<M>(inner: M) -> UrlDecoded<M> {
    UrlDecoded(inner)
}
/// The `UrlDecoded` mapper returned by [url_decoded()](fn.url_decoded.html)
#[derive(Debug)]
pub struct UrlDecoded<M>(M);
impl<IN, M> Matcher<IN> for UrlDecoded<M>
where
    IN: AsRef<[u8]> + ?Sized,
    M: Matcher<[KV<str, str>]>,
{
    fn matches(&mut self, input: &IN, ctx: &mut ExecutionContext) -> bool {
        let decoded: Vec<KV<str, str>> = url::form_urlencoded::parse(input.as_ref())
            .into_owned()
            .map(|(k, v)| KV { k, v })
            .collect();
        ctx.chain(&mut self.0, &decoded)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("UrlDecoded")
            .field(&matcher_name(&self.0))
            .finish()
    }
}

/// json decode the input and pass the resulting value to the inner mapper. If
/// the input cannot be decoded a false value is returned.
///
/// This can be used with Fn matchers to allow for flexible matching of json content.
/// The following example matches whenever the body of the request contains a
/// json list of strings of length 3.
///
/// # Example
///
/// ```rust
/// use httptest::matchers::*;
///
/// request::body(json_decoded(|b: &Vec<String>| b.len() == 3));
/// ```
pub fn json_decoded<T, M>(inner: M) -> JsonDecoded<T, M>
where
    M: Matcher<T>,
{
    JsonDecoded(PhantomData, inner)
}
/// The `JsonDecoded` mapper returned by [json_decoded()](fn.json_decoded.html)
#[derive(Debug)]
pub struct JsonDecoded<T, M>(PhantomData<T>, M);
impl<IN, T, M> Matcher<IN> for JsonDecoded<T, M>
where
    IN: AsRef<[u8]> + ?Sized,
    M: Matcher<T>,
    T: serde::de::DeserializeOwned + fmt::Debug + Send,
{
    fn matches(&mut self, input: &IN, ctx: &mut ExecutionContext) -> bool {
        let value: T = match serde_json::from_slice(input.as_ref()) {
            Ok(value) => value,
            Err(_) => return false,
        };
        ctx.chain(&mut self.1, &value)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("JsonDecoded")
            .field(&matcher_name(&self.1))
            .finish()
    }
}

/// lowercase the input and pass it to the next mapper.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request with a query parameter `foo` in any case.
/// request::query(url_decoded(contains(key(lowercase("foo")))));
/// ```
pub fn lowercase<M>(inner: M) -> Lowercase<M> {
    Lowercase(inner)
}
/// The `Lowercase` mapper returned by [lowercase()](fn.lowercase.html)
#[derive(Debug)]
pub struct Lowercase<M>(M);
impl<IN, M> Matcher<IN> for Lowercase<M>
where
    IN: AsRef<[u8]> + ?Sized,
    M: Matcher<[u8]>,
{
    fn matches(&mut self, input: &IN, ctx: &mut ExecutionContext) -> bool {
        use bstr::ByteSlice;
        ctx.chain(&mut self.0, &input.as_ref().to_lowercase())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Lowercase")
            .field(&matcher_name(&self.0))
            .finish()
    }
}

// Fn(T) -> bool implements Matcher<T>
impl<IN, F> Matcher<IN> for F
where
    F: Fn(&IN) -> bool + Send,
{
    fn matches(&mut self, input: &IN, _ctx: &mut ExecutionContext) -> bool {
        self(input)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "fn(&{}) -> bool", std::any::type_name::<IN>())
    }
}

/// true if any input element matches the provided mapper.
///
/// This works on slices of elements. Each element is handed to the provided
/// mapper until the mapper returns true for one, false if no elements evaluate
/// to true.
///
/// Look at [matches()](fn.matches.html) if substring matching is what want.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request with a header `x-foobar=value`.
/// request::headers(contains(("x-foobar", "value")));
///
/// // A request matcher that matches a request with a query parameter `foo=bar`.
/// request::query(url_decoded(contains(("foo", "bar"))));
///
/// // A request matcher that matches a request with a query parameter `foo` and any value.
/// // Same as `contains(key("foo"))`.
/// request::query(url_decoded(contains(("foo", any()))));
/// ```
pub fn contains<M>(inner: M) -> Contains<M> {
    Contains(inner)
}
/// The `Contains` mapper returned by [contains()](fn.contains.html)
#[derive(Debug)]
pub struct Contains<M>(M);
impl<M, E> Matcher<[E]> for Contains<M>
where
    M: Matcher<E>,
    E: fmt::Debug,
{
    fn matches(&mut self, input: &[E], ctx: &mut ExecutionContext) -> bool {
        input.iter().any(|x| ctx.chain(&mut self.0, x))
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Contains")
            .field(&matcher_name(&self.0))
            .finish()
    }
}

/// extract the key from a key-value pair.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a request with a header `x-foobar` with any value.
/// request::headers(contains(key("x-foobar")));
///
/// // A request matcher that matches a request with a query parameter `foobar` with any value.
/// request::query(url_decoded(contains(key("foobar"))));
/// ```
pub fn key<M>(inner: M) -> Key<M> {
    Key(inner)
}
/// The `Key` mapper returned by [key()](fn.key.html)
#[derive(Debug)]
pub struct Key<M>(M);
impl<M, K, V> Matcher<KV<K, V>> for Key<M>
where
    K: ToOwned + fmt::Debug + ?Sized,
    V: ToOwned + ?Sized,
    M: Matcher<K>,
{
    fn matches(&mut self, input: &KV<K, V>, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, input.k.borrow())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Key").field(&matcher_name(&self.0)).finish()
    }
}

/// extract the value from a key-value pair.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches any query parameter with the value `foobar`.
/// request::query(url_decoded(contains(value("foobar"))));
/// ```
pub fn value<M>(inner: M) -> Value<M> {
    Value(inner)
}
/// The `Value` mapper returned by [value()](fn.value.html)
#[derive(Debug)]
pub struct Value<M>(M);
impl<M, K, V> Matcher<KV<K, V>> for Value<M>
where
    K: ToOwned + ?Sized,
    V: ToOwned + fmt::Debug + ?Sized,
    M: Matcher<V>,
{
    fn matches(&mut self, input: &KV<K, V>, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, input.v.borrow())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Value")
            .field(&matcher_name(&self.0))
            .finish()
    }
}

impl<K, V, KMatcher, VMatcher> Matcher<KV<K, V>> for (KMatcher, VMatcher)
where
    K: ToOwned + fmt::Debug + ?Sized,
    V: ToOwned + fmt::Debug + ?Sized,
    KMatcher: Matcher<K>,
    VMatcher: Matcher<V>,
{
    fn matches(&mut self, input: &KV<K, V>, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, input.k.borrow()) && ctx.chain(&mut self.1, input.v.borrow())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("")
            .field(&matcher_name(&self.0))
            .field(&matcher_name(&self.1))
            .finish()
    }
}

/// extract the length of the input.
///
/// # Example
///
/// ```
/// use httptest::matchers::*;
///
/// // A request matcher that matches a header `x-foobar` and the value has the length of 3.
/// request::headers(contains(("x-foobar", len(eq(3)))));
///
/// // A request matcher that matches a request with two query parameters.
/// request::query(url_decoded(len(eq(2))));
///
/// // A request matcher that matches a body with the length of 42.
/// request::body(len(eq(42)));
/// ```
pub fn len<M>(inner: M) -> Len<M> {
    Len(inner)
}
/// The `Len` mapper returned by [len()](fn.len.html)
#[derive(Debug)]
pub struct Len<M>(M);
impl<M, T> Matcher<[T]> for Len<M>
where
    M: Matcher<usize>,
{
    fn matches(&mut self, input: &[T], ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, &input.len())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Len").field(&matcher_name(&self.0)).finish()
    }
}

impl<M> Matcher<str> for Len<M>
where
    M: Matcher<usize>,
{
    fn matches(&mut self, input: &str, ctx: &mut ExecutionContext) -> bool {
        ctx.chain(&mut self.0, &input.len())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Len").field(&matcher_name(&self.0)).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval<M, I>(matcher: &mut M, input: &I) -> bool
    where
        M: Matcher<I> + ?Sized,
        I: fmt::Debug + ?Sized,
    {
        ExecutionContext::evaluate(matcher, input)
    }

    #[test]
    fn test_eq() {
        let mut c = eq("foo");
        assert_eq!(false, eval(&mut c, "foobar"));
        assert_eq!(false, eval(&mut c, "bazfoobar"));
        assert_eq!(false, eval(&mut c, "bar"));
        assert_eq!(true, eval(&mut c, "foo"));
    }

    #[test]
    fn test_matches() {
        // regex from str
        let mut c = matches(r#"^foo\d*bar$"#);
        assert_eq!(true, eval(&mut c, "foobar"));
        assert_eq!(true, eval(&mut c, "foo99bar"));
        assert_eq!(false, eval(&mut c, "foo99barz"));
        assert_eq!(false, eval(&mut c, "bat"));

        // regex from String
        let mut c = matches(r#"^foo\d*bar$"#.to_owned());
        assert_eq!(true, eval(&mut c, "foobar"));
        assert_eq!(true, eval(&mut c, "foo99bar"));
        assert_eq!(false, eval(&mut c, "foo99barz"));
        assert_eq!(false, eval(&mut c, "bat"));

        // regex from RegexBuilder
        let mut c = matches(regex::bytes::RegexBuilder::new("foobar").case_insensitive(true));
        assert_eq!(true, eval(&mut c, "foobar"));
        assert_eq!(true, eval(&mut c, "FOOBAR"));
        assert_eq!(false, eval(&mut c, "FOO99BAR"));

        // regex from Regex
        let mut c = matches(
            regex::bytes::RegexBuilder::new("foobar")
                .case_insensitive(true)
                .build()
                .unwrap(),
        );
        assert_eq!(true, eval(&mut c, "foobar"));
        assert_eq!(true, eval(&mut c, "FOOBAR"));
        assert_eq!(false, eval(&mut c, "FOO99BAR"));
    }

    #[test]
    fn test_not() {
        let mut c = not(matches(r#"^foo\d*bar$"#));
        assert_eq!(false, eval(&mut c, "foobar"));
        assert_eq!(false, eval(&mut c, "foo99bar"));
        assert_eq!(true, eval(&mut c, "foo99barz"));
        assert_eq!(true, eval(&mut c, "bat"));
    }

    #[test]
    fn test_all_of() {
        let mut c = all_of![matches("foo"), matches("bar")];
        assert_eq!(true, eval(&mut c, "foobar"));
        assert_eq!(true, eval(&mut c, "barfoo"));
        assert_eq!(false, eval(&mut c, "foo"));
        assert_eq!(false, eval(&mut c, "bar"));
    }

    #[test]
    fn test_any_of() {
        let mut c = any_of![matches("foo"), matches("bar")];
        assert_eq!(true, eval(&mut c, "foobar"));
        assert_eq!(true, eval(&mut c, "barfoo"));
        assert_eq!(true, eval(&mut c, "foo"));
        assert_eq!(true, eval(&mut c, "bar"));
        assert_eq!(false, eval(&mut c, "baz"));
    }

    #[test]
    fn test_url_decoded() {
        let expected = vec![KV::new("key 1", "value 1"), KV::new("key2", "")];
        let mut c = request::query(url_decoded(eq(expected)));
        let req = http::Request::get("https://example.com/path?key%201=value%201&key2")
            .body("")
            .unwrap();

        assert_eq!(true, eval(&mut c, &req));
    }

    #[test]
    fn test_json_decoded() {
        let mut c = json_decoded(eq(serde_json::json!({
            "foo": 1,
            "bar": 99,
        })));
        assert_eq!(true, eval(&mut c, r#"{"foo": 1, "bar": 99}"#));
        assert_eq!(true, eval(&mut c, r#"{"bar": 99, "foo": 1}"#));
        assert_eq!(false, eval(&mut c, r#"{"foo": 1, "bar": 100}"#));
    }

    #[test]
    fn test_lowercase() {
        let mut c = lowercase(matches("foo"));
        assert_eq!(true, eval(&mut c, "FOO"));
        assert_eq!(true, eval(&mut c, "FoOBar"));
        assert_eq!(true, eval(&mut c, "foobar"));
        assert_eq!(false, eval(&mut c, "bar"));
    }

    #[test]
    fn test_fn_mapper() {
        let mut c = |input: &u64| input % 2 == 0;
        assert_eq!(true, eval(&mut c, &6));
        assert_eq!(true, eval(&mut c, &20));
        assert_eq!(true, eval(&mut c, &0));
        assert_eq!(false, eval(&mut c, &11));
    }

    #[test]
    fn test_contains() {
        let mut c = contains(eq(100));
        assert_eq!(true, eval(&mut c, vec![100, 200, 300].as_slice()));
        assert_eq!(false, eval(&mut c, vec![99, 200, 300].as_slice()));
    }

    #[test]
    fn test_key() {
        let kv = KV::new("key1", "value1");
        assert_eq!(true, eval(&mut key("key1"), &kv));
        assert_eq!(false, eval(&mut key("key2"), &kv));
    }

    #[test]
    fn test_value() {
        let kv = KV::new("key1", "value1");
        assert_eq!(true, eval(&mut value("value1"), &kv));
        assert_eq!(false, eval(&mut value("value2"), &kv));
    }

    #[test]
    fn test_tuple() {
        let kv = KV::new("key1", "value1");
        assert_eq!(true, eval(&mut ("key1", any()), &kv));
        assert_eq!(true, eval(&mut ("key1", "value1"), &kv));
        assert_eq!(false, eval(&mut ("key1", "value2"), &kv));
        assert_eq!(false, eval(&mut ("key2", "value1"), &kv));
    }

    #[test]
    fn test_len() {
        let mut c = len(eq(3));
        assert_eq!(true, eval(&mut c, "foo"));
        assert_eq!(false, eval(&mut c, "foobar"));
        assert_eq!(true, eval(&mut c, &b"foo"[..]));
        assert_eq!(false, eval(&mut c, &b"foobar"[..]));

        let req = http::Request::get("/test?foo=bar").body("foobar").unwrap();
        assert!(eval(&mut request::body(len(eq(6))), &req));
    }

    #[test]
    fn test_fn() {
        let mut c = len(|&len: &usize| len <= 3);
        assert_eq!(true, eval(&mut c, "f"));
        assert_eq!(true, eval(&mut c, "fo"));
        assert_eq!(true, eval(&mut c, "foo"));
        assert_eq!(false, eval(&mut c, "foob"));
        assert_eq!(false, eval(&mut c, "fooba"));
        assert_eq!(false, eval(&mut c, "foobar"));
    }
}
