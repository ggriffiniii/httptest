use std::borrow::Borrow;
use std::fmt;
use std::marker::PhantomData;

// import the any_of and all_of macros from crate root so they are accessible if
// people glob import this module.
pub use crate::all_of;
pub use crate::any_of;
pub mod request;
pub mod response;

pub trait Mapper<IN>: Send + fmt::Debug
where
    IN: ?Sized,
{
    type Out;

    fn map(&mut self, input: &IN) -> Self::Out;
}

// Matcher is just a special case of Mapper that returns a boolean. Simply
// provides the `matches` method rather than `map` as that reads a little
// better.
pub trait Matcher<IN>: Send + fmt::Debug
where
    IN: ?Sized,
{
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

pub fn any<IN>() -> impl Mapper<IN, Out = bool> {
    Any
}
#[derive(Debug)]
pub struct Any;
impl<IN> Mapper<IN> for Any {
    type Out = bool;

    fn map(&mut self, _input: &IN) -> bool {
        true
    }
}

pub fn contains<T, IN>(value: T) -> impl Mapper<IN, Out = bool>
where
    T: AsRef<[u8]> + fmt::Debug + Send,
    IN: AsRef<[u8]> + ?Sized,
{
    Contains(value)
}
#[derive(Debug)]
pub struct Contains<T>(T);
impl<IN, T> Mapper<IN> for Contains<T>
where
    T: AsRef<[u8]> + fmt::Debug + Send,
    IN: AsRef<[u8]> + ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        use bstr::ByteSlice;
        input.as_ref().contains_str(self.0.as_ref())
    }
}

pub fn eq<T, IN>(value: T) -> impl Mapper<IN, Out = bool>
where
    T: Borrow<IN> + fmt::Debug + Send,
    IN: PartialEq + ?Sized,
{
    Eq(value)
}
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

pub fn matches<IN>(value: &str) -> impl Mapper<IN, Out = bool>
where
    IN: AsRef<[u8]> + ?Sized,
{
    let regex = regex::bytes::Regex::new(value).expect("failed to create regex");
    Matches(regex)
}
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

pub fn not<C, IN>(inner: C) -> impl Mapper<IN, Out = bool>
where
    C: Mapper<IN, Out = bool>,
    IN: ?Sized,
{
    Not(inner, PhantomData)
}
pub struct Not<C, IN>(C, PhantomData<fn(IN)>)
where
    IN: ?Sized;
impl<C, IN> Mapper<IN> for Not<C, IN>
where
    C: Mapper<IN, Out = bool>,
    IN: ?Sized,
{
    type Out = bool;

    fn map(&mut self, input: &IN) -> bool {
        !self.0.map(input)
    }
}
impl<C, IN> fmt::Debug for Not<C, IN>
where
    C: Mapper<IN, Out = bool>,
    IN: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Not({:?})", &self.0)
    }
}

pub fn all_of<IN>(inner: Vec<Box<dyn Mapper<IN, Out = bool>>>) -> impl Mapper<IN, Out = bool>
where
    IN: fmt::Debug + ?Sized,
{
    AllOf(inner)
}

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

pub fn any_of<IN>(inner: Vec<Box<dyn Mapper<IN, Out = bool>>>) -> impl Mapper<IN, Out = bool>
where
    IN: fmt::Debug + ?Sized,
{
    AnyOf(inner)
}
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

pub fn uri_decoded<IN, C>(inner: C) -> impl Mapper<IN, Out = C::Out>
where
    IN: AsRef<[u8]> + ?Sized,
    C: Mapper<[(String, String)]>,
{
    UriDecoded(inner)
}
#[derive(Debug)]
pub struct UriDecoded<C>(C);
impl<IN, C> Mapper<IN> for UriDecoded<C>
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

pub fn json_decoded<IN, C>(inner: C) -> impl Mapper<IN, Out = C::Out>
where
    IN: AsRef<[u8]> + ?Sized,
    C: Mapper<serde_json::Value>,
{
    JsonDecoded(inner)
}
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

pub fn lowercase<IN, C>(inner: C) -> impl Mapper<IN, Out = C::Out>
where
    IN: AsRef<[u8]> + ?Sized,
    C: Mapper<[u8]>,
{
    Lowercase(inner)
}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains() {
        let mut c = contains("foo");
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("bazfoobar"));
        assert_eq!(false, c.map("bar"));
    }

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
        let mut c = matches(r#"^foo\d*bar$"#);
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("foo99bar"));
        assert_eq!(false, c.map("foo99barz"));
        assert_eq!(false, c.map("bat"));
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
        let mut c = all_of![contains("foo"), contains("bar")];
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("barfoo"));
        assert_eq!(false, c.map("foo"));
        assert_eq!(false, c.map("bar"));
    }

    #[test]
    fn test_any_of() {
        let mut c = any_of![contains("foo"), contains("bar")];
        assert_eq!(true, c.map("foobar"));
        assert_eq!(true, c.map("barfoo"));
        assert_eq!(true, c.map("foo"));
        assert_eq!(true, c.map("bar"));
        assert_eq!(false, c.map("baz"));
    }

    #[test]
    fn test_uri_decoded() {
        let expected = vec![
            ("key 1".to_owned(), "value 1".to_owned()),
            ("key2".to_owned(), "".to_owned()),
        ];
        let mut c = request::query(uri_decoded(eq(expected)));
        let req = http::Request::get("https://example.com/path?key%201=value%201&key2")
            .body(Vec::new())
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
        let mut c = lowercase(contains("foo"));
        assert_eq!(true, c.map("FOO"));
        assert_eq!(true, c.map("FoOBar"));
        assert_eq!(true, c.map("foobar"));
        assert_eq!(false, c.map("bar"));
    }
}
