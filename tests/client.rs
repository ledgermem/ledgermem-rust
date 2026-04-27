use ledgermem::{AddMemoryInput, Client, ClientConfig, SearchInput};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn add_memory_sends_auth_and_workspace_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/memories"))
        .and(header("authorization", "Bearer key"))
        .and(header("x-workspace-id", "ws"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "mem_1",
            "content": "hello",
            "createdAt": "2026-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let client = Client::new(ClientConfig {
        api_key: Some("key".into()),
        workspace_id: Some("ws".into()),
        base_url: Some(server.uri()),
        ..Default::default()
    })
    .expect("client builds");

    let mem = client
        .memories()
        .add(AddMemoryInput { content: "hello".into(), ..Default::default() })
        .await
        .expect("add ok");
    assert_eq!(mem.id, "mem_1");
    assert_eq!(mem.content, "hello");
}

#[tokio::test]
async fn search_returns_api_error_on_401() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/search"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(serde_json::json!({"message": "bad key"})),
        )
        .mount(&server)
        .await;

    let client = Client::new(ClientConfig {
        api_key: Some("x".into()),
        workspace_id: Some("ws".into()),
        base_url: Some(server.uri()),
        ..Default::default()
    })
    .expect("client builds");

    let err = client
        .search(SearchInput { query: "hi".into(), ..Default::default() })
        .await
        .expect_err("expected api error");
    match err {
        ledgermem::Error::Api { status, message, .. } => {
            assert_eq!(status, 401);
            assert_eq!(message, "bad key");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
