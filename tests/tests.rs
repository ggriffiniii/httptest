use httptest::{mappers::*, responders::*, Expectation, Times};

async fn read_response_body(resp: hyper::Response<hyper::Body>) -> hyper::Response<Vec<u8>> {
    use futures::stream::TryStreamExt;
    let (head, body) = resp.into_parts();
    let body = body.try_concat().await.unwrap().to_vec();
    hyper::Response::from_parts(head, body)
}

#[tokio::test]
async fn test_server() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![
            request::method(eq("GET")),
            request::path(eq("/foo"))
        ])
        .times(Times::Exactly(1))
        .respond_with(status_code(200)),
    );

    // Issue the GET /foo to the server and verify it returns a 200.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(response::status_code(eq(200)).matches(&resp));

    // The Drop impl of the server will assert that all expectations were satisfied or else it will panic.
}

#[tokio::test]
#[should_panic]
async fn test_expectation_cardinality_not_reached() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![
            request::method(eq("GET")),
            request::path(eq("/foo"))
        ])
        .times(Times::Exactly(1))
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
        Expectation::matching(all_of![
            request::method(eq("GET")),
            request::path(eq("/foo"))
        ])
        .times(Times::Exactly(1))
        .respond_with(
            http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Vec::new())
                .unwrap(),
        ),
    );

    // Issue the GET /foo to the server and verify it returns a 200.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(response::status_code(eq(200)).matches(&resp));

    // Issue a second GET /foo and verify it returns a 500 because the cardinality of the expectation has been exceeded.
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(response::status_code(eq(500)).matches(&resp));

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
        Expectation::matching(all_of![
            request::method(eq("GET")),
            request::path(eq("/foo"))
        ])
        .times(Times::Exactly(1))
        .respond_with(json_encoded(my_data.clone())),
    );

    // Issue the GET /foo to the server and verify it returns a 200.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(all_of![
        response::status_code(eq(200)),
        response::body(json_decoded(eq(my_data))),
    ]
    .matches(&resp));
}

#[tokio::test]
async fn test_cycle() {
    let _ = pretty_env_logger::try_init();

    // Setup a server to expect a single GET /foo request and respond with a
    // json encoding of my_data.
    let server = httptest::Server::run();
    server.expect(
        Expectation::matching(all_of![
            request::method(eq("GET")),
            request::path(eq("/foo"))
        ])
        .times(Times::Exactly(4))
        .respond_with(cycle![status_code(200), status_code(404),]),
    );

    // Issue multiple GET /foo to the server and verify it alternates between 200 and 404 codes.
    let client = hyper::Client::new();
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(response::status_code(eq(200)).matches(&resp));
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(response::status_code(eq(404)).matches(&resp));
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(response::status_code(eq(200)).matches(&resp));
    let resp = read_response_body(client.get(server.url("/foo")).await.unwrap()).await;
    assert!(response::status_code(eq(404)).matches(&resp));
}
