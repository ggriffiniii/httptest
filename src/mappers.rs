//! Mapper implementations.
//!
//! This module contains mappers for composing a set of operations. The result
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

/// The core trait. Defines how an input value should be turned into an output
/// value. This allows for a flexible pattern of composition where two or more
/// mappers are chained together to form a readable and flexible manipulation.
pub trait Mapper<IN>: Send
where
    IN: ?Sized,
{
    /// The output type.
    type Out;

    /// Map an input to output.
    fn map(&mut self, input: &IN) -> Self::Out;

    /// formatted name of the mapper. This is used for debugging purposes and
    /// should typically look like a fmt::Debug representation.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result;
}

/// convenience function to print the Mapper::fmt representation of a mapper.
/// Returns an object with a fmt::Debug matching the Mapper::fmt.
pub(crate) fn mapper_name<M, IN>(mapper: &M) -> MapperName<'_, M, IN>
where
    M: ?Sized,
    IN: ?Sized,
{
    MapperName(mapper, PhantomData)
}
pub(crate) struct MapperName<'a, M, IN>(&'a M, PhantomData<&'a IN>)
where
    M: ?Sized,
    IN: ?Sized;
impl<'a, M, IN> fmt::Debug for MapperName<'a, M, IN>
where
    M: Mapper<IN> + ?Sized,
    IN: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
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

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// true if the input is equal to value.
pub fn eq<T>(value: T) -> Eq<T> {
    Eq(value)
}
/// The `Eq` mapper returned by [eq()](fn.eq.html)
pub struct Eq<T>(T)
where
    T: ?Sized;
impl<IN, T> Mapper<IN> for Eq<T>
where
    T: Borrow<IN> + fmt::Debug + Send + ?Sized,
    IN: PartialEq + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
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
impl<IN> Mapper<IN> for &str
where
    IN: AsRef<[u8]> + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.as_bytes() == input.as_ref()
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// A String is an implicit Eq mapper.
impl<IN> Mapper<IN> for String
where
    IN: AsRef<[u8]> + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.as_bytes() == input.as_ref()
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// A &[u8] is an implicit Eq mapper.
impl<IN> Mapper<IN> for &[u8]
where
    IN: AsRef<[u8]> + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
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

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

/// invert the result of the inner mapper.
pub fn not<M>(inner: M) -> Not<M> {
    Not(inner)
}
/// The `Not` mapper returned by [not()](fn.not.html)
pub struct Not<M>(M);
impl<M, IN> Mapper<IN> for Not<M>
where
    M: Mapper<IN, Out = bool>,
    IN: ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        !self.0.map(input)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Not").field(&mapper_name(&self.0)).finish()
    }
}

/// true if all the provided matchers return true. See the `all_of!` macro for
/// convenient usage.
pub fn all_of<IN>(inner: Vec<Box<dyn Mapper<IN, Out = bool>>>) -> AllOf<IN>
where
    IN: ?Sized,
{
    AllOf(inner)
}

/// The `AllOf` mapper returned by [all_of()](fn.all_of.html)
pub struct AllOf<IN>(Vec<Box<dyn Mapper<IN, Out = bool>>>)
where
    IN: ?Sized;
impl<IN> Mapper<IN> for AllOf<IN>
where
    IN: ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.0.iter_mut().all(|maper| maper.map(input))
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
            .entries(self.0.iter().map(|x| mapper_name(&**x)))
            .finish()
    }
}

/// true if any of the provided matchers returns true. See the `any_of!` macro
/// for convenient usage.
pub fn any_of<IN>(inner: Vec<Box<dyn Mapper<IN, Out = bool>>>) -> AnyOf<IN>
where
    IN: ?Sized,
{
    AnyOf(inner)
}
/// The `AnyOf` mapper returned by [any_of()](fn.any_of.html)
pub struct AnyOf<IN>(Vec<Box<dyn Mapper<IN, Out = bool>>>)
where
    IN: ?Sized;
impl<IN> Mapper<IN> for AnyOf<IN>
where
    IN: ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self.0.iter_mut().any(|maper| maper.map(input))
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
            .entries(self.0.iter().map(|x| mapper_name(&**x)))
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
pub fn url_decoded<M>(inner: M) -> UrlDecoded<M> {
    UrlDecoded(inner)
}
/// The `UrlDecoded` mapper returned by [url_decoded()](fn.url_decoded.html)
#[derive(Debug)]
pub struct UrlDecoded<M>(M);
impl<IN, M> Mapper<IN> for UrlDecoded<M>
where
    IN: AsRef<[u8]> + ?Sized,
    M: Mapper<[KV<str, str>]>,
{
    type Out = M::Out;

    fn map(&mut self, input: &IN) -> M::Out {
        let decoded: Vec<KV<str, str>> = url::form_urlencoded::parse(input.as_ref())
            .into_owned()
            .map(|(k, v)| KV { k, v })
            .collect();
        self.0.map(&decoded)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("UrlDecoded")
            .field(&mapper_name(&self.0))
            .finish()
    }
}

/// json decode the input and pass the resulting value to the inner mapper. If
/// the input cannot be decoded a false value is returned. The inner mapper is
/// required to return a bool value.
///
/// This can be used with Fn mappers to allow for flexible matching of json content.
/// The following example matches whenever the body of the request contains a
/// json list of strings of length 3.
/// ```rust
/// use httptest::mappers::*;
/// request::body(json_decoded(|b: &Vec<String>| b.len() == 3));
/// ```
pub fn json_decoded<T, M>(inner: M) -> JsonDecoded<T, M>
where
    M: Mapper<T, Out = bool>,
{
    JsonDecoded(PhantomData, inner)
}
/// The `JsonDecoded` mapper returned by [json_decoded()](fn.json_decoded.html)
#[derive(Debug)]
pub struct JsonDecoded<T, M>(PhantomData<T>, M);
impl<IN, T, M> Mapper<IN> for JsonDecoded<T, M>
where
    IN: AsRef<[u8]> + ?Sized,
    M: Mapper<T, Out = bool>,
    T: serde::de::DeserializeOwned + Send,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        let value: T = match serde_json::from_slice(input.as_ref()) {
            Ok(value) => value,
            Err(_) => return false,
        };
        self.1.map(&value)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("JsonDecoded")
            .field(&mapper_name(&self.1))
            .finish()
    }
}

/// lowercase the input and pass it to the next mapper.
pub fn lowercase<M>(inner: M) -> Lowercase<M> {
    Lowercase(inner)
}
/// The `Lowercase` mapper returned by [lowercase()](fn.lowercase.html)
#[derive(Debug)]
pub struct Lowercase<M>(M);
impl<IN, M> Mapper<IN> for Lowercase<M>
where
    IN: AsRef<[u8]> + ?Sized,
    M: Mapper<[u8]>,
{
    type Out = M::Out;

    fn map(&mut self, input: &IN) -> M::Out {
        use bstr::ByteSlice;
        self.0.map(&input.as_ref().to_lowercase())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Lowercase")
            .field(&mapper_name(&self.0))
            .finish()
    }
}

// Fn(T) -> bool implements Mapper<T>
impl<IN, F> Mapper<IN> for F
where
    F: Fn(&IN) -> bool + Send,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        self(input)
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "fn(&{}) -> bool", std::any::type_name::<IN>())
    }
}

/// true if any input element matches the provided mapper.
///
/// This works on slices of elements. Each element is handed to the provided
/// mapper until the mapper returns true for one, false if no elements evaulate
/// to true.
///
/// Look at [matches()](fn.matches.html) if substring matching is what want.
pub fn contains<M>(inner: M) -> Contains<M> {
    Contains(inner)
}
/// The `Contains` mapper returned by [contains()](fn.contains.html)
#[derive(Debug)]
pub struct Contains<M>(M);
impl<M, E> Mapper<[E]> for Contains<M>
where
    M: Mapper<E, Out = bool>,
{
    type Out = bool;

    fn map(&mut self, input: &[E]) -> bool {
        input.iter().any(|x| self.0.map(x))
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Contains")
            .field(&mapper_name(&self.0))
            .finish()
    }
}

/// extract the key from a key-value pair.
pub fn key<M>(inner: M) -> Key<M> {
    Key(inner)
}
/// The `Key` mapper returned by [key()](fn.key.html)
#[derive(Debug)]
pub struct Key<M>(M);
impl<M, K, V> Mapper<KV<K, V>> for Key<M>
where
    K: ToOwned + ?Sized,
    V: ToOwned + ?Sized,
    M: Mapper<K>,
{
    type Out = M::Out;

    fn map(&mut self, input: &KV<K, V>) -> M::Out {
        self.0.map(input.k.borrow())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Key").field(&mapper_name(&self.0)).finish()
    }
}

/// extract the value from a key-value pair.
pub fn value<M>(inner: M) -> Value<M> {
    Value(inner)
}
/// The `Value` mapper returned by [value()](fn.value.html)
#[derive(Debug)]
pub struct Value<M>(M);
impl<M, K, V> Mapper<KV<K, V>> for Value<M>
where
    K: ToOwned + ?Sized,
    V: ToOwned + ?Sized,
    M: Mapper<V>,
{
    type Out = M::Out;

    fn map(&mut self, input: &KV<K, V>) -> M::Out {
        self.0.map(input.v.borrow())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Value").field(&mapper_name(&self.0)).finish()
    }
}

impl<K, V, KMapper, VMapper> Mapper<KV<K, V>> for (KMapper, VMapper)
where
    K: ToOwned + ?Sized,
    V: ToOwned + ?Sized,
    KMapper: Mapper<K, Out = bool>,
    VMapper: Mapper<V, Out = bool>,
{
    type Out = bool;

    fn map(&mut self, input: &KV<K, V>) -> bool {
        self.0.map(input.k.borrow()) && self.1.map(input.v.borrow())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("")
            .field(&mapper_name(&self.0))
            .field(&mapper_name(&self.1))
            .finish()
    }
}

/// extract the length of the input.
pub fn len<M>(inner: M) -> Len<M> {
    Len(inner)
}
/// The `Len` mapper returned by [len()](fn.len.html)
#[derive(Debug)]
pub struct Len<M>(M);
impl<M, T> Mapper<[T]> for Len<M>
where
    M: Mapper<usize>,
{
    type Out = M::Out;

    fn map(&mut self, input: &[T]) -> M::Out {
        self.0.map(&input.len())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Len").field(&mapper_name(&self.0)).finish()
    }
}

impl<M> Mapper<str> for Len<M>
where
    M: Mapper<usize>,
{
    type Out = M::Out;

    fn map(&mut self, input: &str) -> M::Out {
        self.0.map(&input.len())
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Len").field(&mapper_name(&self.0)).finish()
    }
}

/// inspect the input and pass it to the next mapper.
///
/// This logs the value as it passes it to the next mapper unchanged. Can be
/// useful when troubleshooting why a matcher may not be working as intended.
pub fn inspect<M>(inner: M) -> Inspect<M> {
    Inspect(inner)
}
/// The `Inspect` mapper returned by [inspect()](fn.inspect.html)
#[derive(Debug)]
pub struct Inspect<M>(M);
impl<IN, M> Mapper<IN> for Inspect<M>
where
    IN: fmt::Debug + ?Sized,
    M: Mapper<IN>,
    M::Out: fmt::Debug,
{
    type Out = M::Out;

    fn map(&mut self, input: &IN) -> M::Out {
        let output = self.0.map(input);
        log::debug!(
            "{:?}.map({:?}) == {:?}",
            mapper_name(&self.0),
            input,
            output
        );
        output
    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Inspect")
            .field(&mapper_name(&self.0))
            .finish()
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
        let expected = vec![KV::new("key 1", "value 1"), KV::new("key2", "")];
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
        let mut c = |input: &u64| input % 2 == 0;
        assert_eq!(true, c.map(&6));
        assert_eq!(true, c.map(&20));
        assert_eq!(true, c.map(&0));
        assert_eq!(false, c.map(&11));
    }

    #[test]
    fn test_contains() {
        let mut c = contains(eq(100));
        assert_eq!(true, c.map(vec![100, 200, 300].as_slice()));
        assert_eq!(false, c.map(vec![99, 200, 300].as_slice()));
    }

    #[test]
    fn test_key() {
        let kv = KV::new("key1", "value1");
        assert_eq!(true, key("key1").map(&kv));
        assert_eq!(false, key("key2").map(&kv));
    }

    #[test]
    fn test_value() {
        let kv = KV::new("key1", "value1");
        assert_eq!(true, value("value1").map(&kv));
        assert_eq!(false, value("value2").map(&kv));
    }

    #[test]
    fn test_tuple() {
        let kv = KV::new("key1", "value1");
        assert_eq!(true, ("key1", any()).map(&kv));
        assert_eq!(true, ("key1", "value1").map(&kv));
        assert_eq!(false, ("key1", "value2").map(&kv));
        assert_eq!(false, ("key2", "value1").map(&kv));
    }

    #[test]
    fn test_len() {
        let mut c = len(eq(3));
        assert_eq!(true, c.map("foo"));
        assert_eq!(false, c.map("foobar"));
        assert_eq!(true, c.map(&b"foo"[..]));
        assert_eq!(false, c.map(&b"foobar"[..]));
    }

    #[test]
    fn test_fn() {
        let mut c = len(|&len: &usize| len <= 3);
        assert_eq!(true, c.map("f"));
        assert_eq!(true, c.map("fo"));
        assert_eq!(true, c.map("foo"));
        assert_eq!(false, c.map("foob"));
        assert_eq!(false, c.map("fooba"));
        assert_eq!(false, c.map("foobar"));
    }

    #[test]
    fn test_inspect() {
        let _ = pretty_env_logger::try_init();
        let mut c = inspect(lowercase(matches("^foobar$")));
        assert_eq!(true, c.map("Foobar"));
        assert_eq!(false, c.map("Foobar1"));
    }
}
