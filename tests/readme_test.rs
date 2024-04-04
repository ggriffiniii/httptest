#[tokio::test]
async fn test_readme() {
    use http_body_util::{BodyExt, Full};
    use httptest::{matchers::*, responders::*, Expectation, Server};
    use hyper_util::client::legacy::Client;
    use serde_json::json;
    // Starting a logger within the test can make debugging a failed test
    // easier. The mock http server will log::debug every request and response
    // received along with what, if any, matcher was found for the request. When
    // env_logger is initialized running the test with `RUST_LOG=httptest=debug
    // cargo test` can provide that information on stderr.
    let _ = pretty_env_logger::try_init();
    // Start a server running on a local ephemeral port.
    let server = Server::run();
    // Configure the server to expect a single GET /foo request and respond
    // with a 200 status code.
    server.expect(
        Expectation::matching(request::method_path("GET", "/foo")).respond_with(status_code(200)),
    );
    // Configure the server to also receive between 1 and 3 POST /bar requests
    // with a json body matching {'foo': 'bar'}, and respond with a json body
    // {'result': 'success'}
    server.expect(
        Expectation::matching(all_of![
            request::method("POST"),
            request::path("/bar"),
            request::body(json_decoded(eq(json!({"foo": "bar"})))),
        ])
        .times(1..=3)
        .respond_with(json_encoded(json!({"result": "success"}))),
    );

    // The server provides server.addr() that returns the address of the
    // locally running server, or more conveniently provides a server.url()
    // method that gives a fully formed http url to the provided path.
    let url = server.url("/foo");

    // Now test your http client against the server.
    let client = Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http::<Full<hyper::body::Bytes>>();

    // Issue the GET /foo to the server.
    let resp = client.get(url).await.unwrap();
    // Optionally use response matchers to assert the server responded as
    // expected.

    // Assert the response was a 200.
    assert_eq!(200, resp.status().as_u16());

    // Issue a POST /bar with {'foo': 'bar'} json body.
    let post_req = http::Request::post(server.url("/bar"))
        .body(json!({"foo": "bar"}).to_string().into())
        .unwrap();
    let resp = client.request(post_req).await.unwrap();

    // Assert the response was a 200 with a json body of {'result': 'success'}
    assert_eq!(200, resp.status().as_u16());

    // Read the entire response body into a Vec<u8> to allow using the body
    // response matcher.
    let body = resp.collect().await.unwrap().to_bytes();
    assert_eq!(
        json!({"result": "success"}),
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()
    );

    // on Drop the server will assert all expectations have been met and will
    // panic if not.
}
