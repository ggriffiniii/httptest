//! Mapper implementations.
//!
//! This module contains mappers for composing a set of operations. The result
//! of the composition usually results in a boolean. Any `Mapper` that results in a
//! boolean value also implemens `Matcher`.

use std::borrow::Borrow;
use std::fmt;

// import the any_of and all_of macros from crate root so they are accessible if
// people glob import this module.
/// Accept a list of matchers and returns true if all matchers are true.
#[doc(inline)]
pub use crate::all_of;
/// Accept a list of matchers and returns true if any matcher is true.
#[doc(inline)]
pub use crate::any_of;

pub mod request;
pub mod response;
pub mod sequence;

/// The core trait. Defines how an input value should be turned into an output
/// value. This allows for a flexible pattern of composition where two or more
/// mappers are chained together to form a readable and flexible manipulation.
///
/// There is a special case of a Mapper that outputs a bool that is called a
/// Matcher.
pub trait Mapper<IN>: Send + fmt::Debug
where
    IN: ?Sized,
{
    /// The output type.
    type Out;

    /// Map an input to output.
    fn map(&mut self, input: &IN) -> Self::Out;
}

/// Matcher is just a special case of Mapper that returns a boolean. It simply
/// provides the `matches` method rather than `map` as that reads a little
/// better.
///
/// There is a blanket implementation for all Mappers that output bool values.
/// You should never implement Matcher yourself, instead implement Mapper with a
/// bool Out parameter.
pub trait Matcher<IN>: Send + fmt::Debug
where
    IN: ?Sized,
{
    /// true if the input matches.
    fn matches(&mut self, input: &IN) -> bool;
}
impl<T, IN> Matcher<IN> for T
where
    T: Mapper<IN, Out = bool>,
{
    fn matches(&mut self, input: &IN) -> bool {
        self.map(input)
    }
}

/// Always true.
pub fn any() -> Any {
    Any
}
/// The `Any` mapper returned by [any()](fn.any.html)
#[derive(Debug)]
pub struct Any;
impl<IN> Mapper<IN> for Any
where
    IN: ?Sized,
{
    type Out = bool;

    fn map(&mut self, _input: &IN) -> bool {
        true
    }
}

/// true if the input is equal to value.
pub fn eq<T>(value: T) -> Eq<T> {
    Eq(value)
}
/// The `Eq` mapper returned by [eq()](fn.eq.html)
#[derive(Debug)]
pub struct Eq<T>(T);
impl<IN, T> Mapper<IN> for Eq<T>
where
    T: Borrow<IN> + fmt::Debug + Send,
    IN: PartialEq + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.0.borrow() == input
    }
}

/// Call Deref::deref() on the input and pass it to the next mapper.
pub fn deref<C>(inner: C) -> Deref<C> {
    Deref(inner)
}
/// The `Deref` mapper returned by [deref()](fn.deref.html)
#[derive(Debug)]
pub struct Deref<C>(C);
impl<C, IN> Mapper<IN> for Deref<C>
where
    C: Mapper<IN::Target>,
    IN: std::ops::Deref,
{
    type Out = C::Out;

    fn map(&mut self, input: &IN) -> C::Out {
        self.0.map(input.deref())
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
pub fn matches(value: impl IntoRegex) -> Matches {
    //let regex = regex::bytes::Regex::new(value).expect("failed to create regex");
    Matches(value.into_regex())
}
/// The `Matches` mapper returned by [matches()](fn.matches.html)
#[derive(Debug)]
pub struct Matches(regex::bytes::Regex);
impl<IN> Mapper<IN> for Matches
where
    IN: AsRef<[u8]> + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.0.is_match(input.as_ref())
    }
}

/// invert the result of the inner mapper.
pub fn not<C>(inner: C) -> Not<C> {
    Not(inner)
}
/// The `Not` mapper returned by [not()](fn.not.html)
pub struct Not<C>(C);
impl<C, IN> Mapper<IN> for Not<C>
where
    C: Mapper<IN, Out = bool>,
    IN: ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        !self.0.map(input)
    }
}
impl<C> fmt::Debug for Not<C>
where
    C: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Not({:?})", &self.0)
    }
}

/// true if all the provided matchers return true. See the `all_of!` macro for
/// convenient usage.
pub fn all_of<IN>(inner: Vec<Box<dyn Mapper<IN, Out = bool>>>) -> AllOf<IN>
where
    IN: fmt::Debug + ?Sized,
{
    AllOf(inner)
}

/// The `AllOf` mapper returned by [all_of()](fn.all_of.html)
#[derive(Debug)]
pub struct AllOf<IN>(Vec<Box<dyn Mapper<IN, Out = bool>>>)
where
    IN: ?Sized;
impl<IN> Mapper<IN> for AllOf<IN>
where
    IN: fmt::Debug + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.0.iter_mut().all(|maper| maper.map(input))
    }
}

/// true if any of the provided matchers returns true. See the `any_of!` macro
/// for convenient usage.
pub fn any_of<IN>(inner: Vec<Box<dyn Mapper<IN, Out = bool>>>) -> AnyOf<IN>
where
    IN: fmt::Debug + ?Sized,
{
    AnyOf(inner)
}
/// The `AnyOf` mapper returned by [any_of()](fn.any_of.html)
#[derive(Debug)]
pub struct AnyOf<IN>(Vec<Box<dyn Mapper<IN, Out = bool>>>)
where
    IN: ?Sized;
impl<IN> Mapper<IN> for AnyOf<IN>
where
    IN: fmt::Debug + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.0.iter_mut().any(|maper| maper.map(input))
    }
}

/// url decode the input and pass the resulting slice of key-value pairs to the next mapper.
pub fn url_decoded<C>(inner: C) -> UrlDecoded<C> {
    UrlDecoded(inner)
}
/// The `UrlDecoded` mapper returned by [url_decoded()](fn.url_decoded.html)
#[derive(Debug)]
pub struct UrlDecoded<C>(C);
impl<IN, C> Mapper<IN> for UrlDecoded<C>
where
    IN: AsRef<[u8]> + ?Sized,
    C: Mapper<[(String, String)]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &IN) -> C::Out {
        let decoded: Vec<(String, String)> = url::form_urlencoded::parse(input.as_ref())
            .into_owned()
            .collect();
        self.0.map(&decoded)
    }
}

/// json decode the input and pass the resulting serde_json::Value to the next
/// mapper.
///
/// If the input can't be decoded a serde_json::Value::Null is passed to the next
/// mapper.
pub fn json_decoded<C>(inner: C) -> JsonDecoded<C> {
    JsonDecoded(inner)
}
/// The `JsonDecoded` mapper returned by [json_decoded()](fn.json_decoded.html)
#[derive(Debug)]
pub struct JsonDecoded<C>(C);
impl<IN, C> Mapper<IN> for JsonDecoded<C>
where
    IN: AsRef<[u8]> + ?Sized,
    C: Mapper<serde_json::Value>,
{
    type Out = C::Out;

    fn map(&mut self, input: &IN) -> C::Out {
        let json_value: serde_json::Value =
            serde_json::from_slice(input.as_ref()).unwrap_or(serde_json::Value::Null);
        self.0.map(&json_value)
    }
}

/// lowercase the input and pass it to the next mapper.
pub fn lowercase<C>(inner: C) -> Lowercase<C>
where
    C: Mapper<[u8]>,
{
    Lowercase(inner)
}
/// The `Lowercase` mapper returned by [lowercase()](fn.lowercase.html)
#[derive(Debug)]
pub struct Lowercase<C>(C);
impl<IN, C> Mapper<IN> for Lowercase<C>
where
    IN: AsRef<[u8]> + ?Sized,
    C: Mapper<[u8]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &IN) -> C::Out {
        use bstr::ByteSlice;
        self.0.map(&input.as_ref().to_lowercase())
    }
}

/// pass the input to the provided `Fn(T) -> bool` and return the result.
pub fn map_fn<F>(f: F) -> MapFn<F> {
    MapFn(f)
}
/// The `MapFn` mapper returned by [map_fn()](fn.map_fn.html)
pub struct MapFn<F>(F);
impl<IN, F> Mapper<IN> for MapFn<F>
where
    F: Fn(&IN) -> bool + Send,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.0(input)
    }
}
impl<F> fmt::Debug for MapFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MapFn")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq() {
        let mut c = eq("foo");
        assert_eq!(false, c.map("foobar"));
        assert_eq!(false, c.map("bazfoobar"));
        assert_eq!(false, c.map("bar"));
        assert_eq!(true, c.map("foo"));
    }

    #[test]
    fn test_matches() {
        // regex from str
        let mut c = matches(r#"^foo\d*bar$"#);
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("foo99bar"));
        assert_eq!(false, c.map("foo99barz"));
        assert_eq!(false, c.map("bat"));

        // regex from String
        let mut c = matches(r#"^foo\d*bar$"#.to_owned());
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("foo99bar"));
        assert_eq!(false, c.map("foo99barz"));
        assert_eq!(false, c.map("bat"));

        // regex from RegexBuilder
        let mut c = matches(regex::bytes::RegexBuilder::new("foobar").case_insensitive(true));
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("FOOBAR"));
        assert_eq!(false, c.map("FOO99BAR"));

        // regex from Regex
        let mut c = matches(
            regex::bytes::RegexBuilder::new("foobar")
                .case_insensitive(true)
                .build()
                .unwrap(),
        );
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("FOOBAR"));
        assert_eq!(false, c.map("FOO99BAR"));
    }

    #[test]
    fn test_not() {
        let mut c = not(matches(r#"^foo\d*bar$"#));
        assert_eq!(false, c.map("foobar"));
        assert_eq!(false, c.map("foo99bar"));
        assert_eq!(true, c.map("foo99barz"));
        assert_eq!(true, c.map("bat"));
    }

    #[test]
    fn test_all_of() {
        let mut c = all_of![matches("foo"), matches("bar")];
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("barfoo"));
        assert_eq!(false, c.map("foo"));
        assert_eq!(false, c.map("bar"));
    }

    #[test]
    fn test_any_of() {
        let mut c = any_of![matches("foo"), matches("bar")];
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("barfoo"));
        assert_eq!(true, c.map("foo"));
        assert_eq!(true, c.map("bar"));
        assert_eq!(false, c.map("baz"));
    }

    #[test]
    fn test_url_decoded() {
        let expected = vec![
            ("key 1".to_owned(), "value 1".to_owned()),
            ("key2".to_owned(), "".to_owned()),
        ];
        let mut c = request::query(url_decoded(eq(expected)));
        let req = http::Request::get("https://example.com/path?key%201=value%201&key2")
            .body("")
            .unwrap();

        assert_eq!(true, c.map(&req));
    }

    #[test]
    fn test_json_decoded() {
        let mut c = json_decoded(eq(serde_json::json!({
            "foo": 1,
            "bar": 99,
        })));
        assert_eq!(true, c.map(r#"{"foo": 1, "bar": 99}"#));
        assert_eq!(true, c.map(r#"{"bar": 99, "foo": 1}"#));
        assert_eq!(false, c.map(r#"{"foo": 1, "bar": 100}"#));
    }

    #[test]
    fn test_lowercase() {
        let mut c = lowercase(matches("foo"));
        assert_eq!(true, c.map("FOO"));
        assert_eq!(true, c.map("FoOBar"));
        assert_eq!(true, c.map("foobar"));
        assert_eq!(false, c.map("bar"));
    }

    #[test]
    fn test_fn_mapper() {
        let mut c = map_fn(|input: &u64| input % 2 == 0);
        assert_eq!(true, c.map(&6));
        assert_eq!(true, c.map(&20));
        assert_eq!(true, c.map(&0));
        assert_eq!(false, c.map(&11));
    }
}
