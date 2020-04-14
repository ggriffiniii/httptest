use httptest::{matchers::*, responders::*, Expectation};
use std::future::Future;

async fn read_response_body(
    resp_fut: impl Future<Output = Result<hyper::Response<hyper::Body>, hyper::Error>>,
) -> hyper::Response<hyper::body::Bytes> {
    let resp = resp_fut.await.unwrap();
    let (head, body) = resp.into_parts();
    let body = hyper::body::to_bytes(body).await.unwrap();
    hyper::Response::from_parts(head, body)
}

#[tokio::test]
async fn test_server() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![request::method("GET"), request::path("/foo")])
            .respond_with(status_code(200)),
    );

    // Issue the GET /foo to the server and verify it returns a 200.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert_eq!(200, resp.status().as_u16());

    // The Drop impl of the server will assert that all expectations were satisfied or else it will panic.
}

#[tokio::test]
#[should_panic]
async fn test_expectation_cardinality_not_reached() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![request::method("GET"), request::path("/foo")])
            .respond_with(status_code(200)),
    );

    // Don't send any requests. Should panic.
}

#[tokio::test]
#[should_panic]
async fn test_expectation_cardinality_exceeded() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![request::method("GET"), request::path("/foo")]).respond_with(
            http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Vec::new())
                .unwrap(),
        ),
    );

    // Issue the GET /foo to the server and verify it returns a 200.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert_eq!(200, resp.status().as_u16());

    // Issue a second GET /foo and verify it returns a 500 because the cardinality of the expectation has been exceeded.
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert!(resp.status().is_server_error());

    // Should panic on Server drop.
}

#[tokio::test]
async fn test_json() {
    let _ = pretty_env_logger::try_init();

    let my_data = serde_json::json!({
        "foo": "bar",
        "baz": [1, 2, 3],
    });

    // Setup a server to expect a single GET /foo request and respond with a
    // json encoding of my_data.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![request::method("GET"), request::path("/foo")])
            .respond_with(json_encoded(my_data.clone())),
    );

    // Issue the GET /foo to the server and verify it returns a 200 with a json
    // body matching my_data.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert_eq!(200, resp.status().as_u16());
    let body_data = serde_json::from_slice::<serde_json::Value>(resp.body()).unwrap();
    assert_eq!(my_data, body_data);
    assert_eq!(
        Some(&b"application/json"[..]),
        resp.headers().get("content-type").map(|x| x.as_bytes())
    );
}

#[tokio::test]
async fn test_cycle() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request and respond with a
    // json encoding of my_data.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![request::method("GET"), request::path("/foo")])
            .times(4)
            .respond_with(cycle![status_code(200), status_code(404),]),
    );

    // Issue multiple GET /foo to the server and verify it alternates between 200 and 404 codes.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert_eq!(200, resp.status().as_u16());
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert_eq!(404, resp.status().as_u16());
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert_eq!(200, resp.status().as_u16());
    let resp = read_response_body(client.get(server.url("/foo"))).await;
    assert_eq!(404, resp.status().as_u16());
}

#[tokio::test]
async fn test_url_encoded() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request and respond with a
    // json response.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![
            request::method("GET"),
            request::path("/foo"),
            request::query(url_decoded(contains(("key", "value")))),
        ])
        .respond_with(url_encoded(&[("key", "value"), ("k", "v")])),
    );

    // Issue the GET /foo?key=value to the server and verify it returns a 200 with an
    // application/x-www-form-urlencoded body of key=value.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo?key=value"))).await;
    assert_eq!(200, resp.status().as_u16());
    assert_eq!(
        Some(&b"application/x-www-form-urlencoded"[..]),
        resp.headers().get("content-type").map(|x| x.as_bytes())
    );
    assert_eq!("key=value&k=v", resp.body());

    // The Drop impl of the server will assert that all expectations were satisfied or else it will panic.
}

#[tokio::test]
async fn test_respond_with_fn() {
    let _ = pretty_env_logger::try_init();

    let server = httptest::Server::run();
    let delay = std::time::Duration::from_millis(100);
    server.expect(Expectation::matching(any()).respond_with(move || {
        std::thread::sleep(delay);
        status_code(200)
    }));

    // Issue the GET /foo?key=value to the server and verify it returns a 200 with an
    // application/x-www-form-urlencoded body of key=value.
    let client = hyper::Client::new();
    let now = std::time::Instant::now();
    let resp = read_response_body(client.get(server.url("/foo?key=value"))).await;
    let elapsed = now.elapsed();
    assert_eq!(200, resp.status().as_u16());
    assert!(elapsed >= delay);

    // The Drop impl of the server will assert that all expectations were satisfied or else it will panic.
}

#[tokio::test]
async fn test_custom_json() {
    use httptest::{matchers::*, responders::*, Expectation, Server};
    use serde_json::json;
    let _ = pretty_env_logger::try_init();

    let server = Server::run();

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct PostBody {
        msg: Option<String>,
    }

    server.expect(
        Expectation::matching(all_of![
            request::method("POST"),
            request::path("/bar"),
            request::body(json_decoded(|b: &PostBody| { b.msg.is_some() }))
        ])
        .respond_with(json_encoded(json!({"result": "success"}))),
    );

    // Now test your http client against the server.
    let client = hyper::Client::new();
    // Issue the GET /foo to the server.

    // Issue a POST /bar with {'foo': 'bar'} json body.
    let post_req = http::Request::post(server.url("/bar"))
        .body(json!({"msg": "foo"}).to_string().into())
        .unwrap();
    // Read the entire response body into a Vec<u8> to allow using the body
    // response matcher.
    let resp = read_response_body(client.request(post_req)).await;
    // Assert the response was a 200 with a json body of {'result': 'success'}
    assert_eq!(200, resp.status().as_u16());
    assert_eq!(
        json!({"result": "success"}),
        serde_json::from_slice::<serde_json::Value>(resp.body()).unwrap()
    );

    // on Drop the server will assert all expectations have been met and will
    // panic if not.
}

#[tokio::test]
async fn test_readme() {
    use httptest::{matchers::*, responders::*, Expectation, Server};
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
    let client = hyper::Client::new();
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
    // Read the entire response body into a Vec<u8> to allow using the body
    // response matcher.
    let resp = read_response_body(client.request(post_req)).await;
    // Assert the response was a 200 with a json body of {'result': 'success'}
    assert_eq!(200, resp.status().as_u16());
    assert_eq!(
        json!({"result": "success"}),
        serde_json::from_slice::<serde_json::Value>(resp.body()).unwrap()
    );

    // on Drop the server will assert all expectations have been met and will
    // panic if not.
}

// verify that the server can be started even if not run within a tokio context.
#[test]
fn test_outside_of_tokio_context() {
    let _ = pretty_env_logger::try_init();
    let _server = httptest::Server::run();
}
