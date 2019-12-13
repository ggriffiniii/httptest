# httptest

Provide convenient mechanism for testing http clients against a locally
running http server. The typical usage is as follows:

* Start a server
* Configure the server by adding expectations
* Test your http client by making requests to the server
* On Drop the server verifies all expectations were met.

## Example Test

```rust
#[tokio::test]
async fn test_readme() {
    use httptest::{mappers::*, responders::*, Expectation, Server, Times};
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
        Expectation::matching(all_of![
            request::method(eq("GET")),
            request::path(eq("/foo"))
        ])
        .times(Times::Exactly(1))
        .respond_with(status_code(200)),
    );
    // Configure the server to also receive between 1 and 3 POST /bar requests
    // with a json body matching {'foo': 'bar'}, and respond with a json body
    // {'result': 'success'}
    server.expect(
        Expectation::matching(all_of![
            request::method(eq("POST")),
            request::path(eq("/bar")),
            request::body(json_decoded(eq(json!({"foo": "bar"})))),
        ])
        .times(Times::Between(1..=3))
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
    assert!(response::status_code(eq(200)).matches(&resp));

    // Issue a POST /bar with {'foo': 'bar'} json body.
    let post_req = http::Request::post(server.url("/bar"))
        .body(json!({"foo": "bar"}).to_string().into())
        .unwrap();
    // Read the entire response body into a Vec<u8> to allow using the body
    // response matcher.
    let resp = read_response_body(client.request(post_req)).await;
    // Assert the response was a 200 with a json body of {'result': 'success'}
    assert!(all_of![
        response::status_code(eq(200)),
        response::body(json_decoded(eq(json!({"result": "success"})))),
    ]
    .matches(&resp));

    // on Drop the server will assert all expectations have been met and will
    // panic if not.
}
```
