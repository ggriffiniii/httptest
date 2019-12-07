use super::Mapper;
use crate::FullResponse;

pub fn status_code<C>(inner: C) -> impl Mapper<FullResponse, Out = C::Out>
where
    C: Mapper<u16>,
{
    StatusCode(inner)
}
#[derive(Debug)]
pub struct StatusCode<C>(C);
impl<C> Mapper<FullResponse> for StatusCode<C>
where
    C: Mapper<u16>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullResponse) -> C::Out {
        self.0.map(&input.status().as_u16())
    }
}

pub fn headers<C>(inner: C) -> impl Mapper<FullResponse, Out = C::Out>
where
    C: Mapper<[(Vec<u8>, Vec<u8>)]>,
{
    Headers(inner)
}
#[derive(Debug)]
pub struct Headers<C>(C);
impl<C> Mapper<FullResponse> for Headers<C>
where
    C: Mapper<[(Vec<u8>, Vec<u8>)]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullResponse) -> C::Out {
        let headers: Vec<(Vec<u8>, Vec<u8>)> = input
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().into(), v.as_bytes().into()))
            .collect();
        self.0.map(&headers)
    }
}

pub fn body<C>(inner: C) -> impl Mapper<FullResponse, Out = C::Out>
where
    C: Mapper<[u8]>,
{
    Body(inner)
}
#[derive(Debug)]
pub struct Body<C>(C);
impl<C> Mapper<FullResponse> for Body<C>
where
    C: Mapper<[u8]>,
{
    type Out = C::Out;

    fn map(&mut self, input: &FullResponse) -> C::Out {
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
            .body(Vec::new())
            .unwrap();
        assert!(status_code(eq(404)).map(&resp));

        let resp = hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .body(Vec::new())
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
            .body(Vec::new())
            .unwrap();

        assert!(headers(eq(expected)).map(&resp));
    }

    #[test]
    fn test_body() {
        use bstr::{ByteVec, B};
        let resp = hyper::Response::builder()
            .body(Vec::from_slice("my request body"))
            .unwrap();
        assert!(body(eq(B("my request body"))).map(&resp));
    }
}
