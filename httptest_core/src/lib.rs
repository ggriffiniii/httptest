use std::fmt;
use std::pin::Pin;
use std::future::Future;

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

// Any tuple of two matchers returning a bool returns the AND of the results.
impl<K, V, KMapper, VMapper> Mapper<(K, V)> for (KMapper, VMapper)
where
    KMapper: Mapper<K, Out = bool>,
    VMapper: Mapper<V, Out = bool>,
{
    type Out = bool;

    fn map(&mut self, input: &(K, V)) -> bool {
        self.0.map(&input.0) && self.1.map(&input.1)
    }
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

/// Respond with an HTTP response.
pub trait Responder: Send + fmt::Debug {
    /// Return a future that outputs an HTTP response.
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<Vec<u8>>> + Send>>;
}

// Implement Responder for any http::Response<B> where B can be turned into a Vec<u8>.
impl<B> Responder for http::Response<B>
where
    B: Into<Vec<u8>> + Clone + Send + fmt::Debug,
{
    fn respond(&mut self) -> Pin<Box<dyn Future<Output = http::Response<Vec<u8>>> + Send>> {
        async fn _respond(resp: http::Response<Vec<u8>>) -> http::Response<Vec<u8>> {
            resp
        }
        let mut builder = http::Response::builder();
        builder = builder
            .status(self.status().clone())
            .version(self.version().clone());
        *builder.headers_mut().unwrap() = self.headers().clone();
        let resp = builder.body(self.body().clone().into()).unwrap();

        Box::pin(_respond(resp))
    }
}

