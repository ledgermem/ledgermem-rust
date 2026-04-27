# ledgermem

Official Rust SDK for [LedgerMem](https://proofly.dev) — auditable memory for AI agents.

## Install

```toml
[dependencies]
ledgermem = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Requires Rust 1.75+.

## Quickstart

```rust
use ledgermem::{Client, ClientConfig, AddMemoryInput, SearchInput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new(ClientConfig {
        api_key: Some("lm_live_...".into()),
        workspace_id: Some("ws_123".into()),
        ..Default::default()
    })?;

    let mem = client.memories().add(AddMemoryInput {
        content: "User prefers dark mode.".into(),
        ..Default::default()
    }).await?;

    let result = client.search(SearchInput {
        query: "dark mode".into(),
        limit: Some(5),
        ..Default::default()
    }).await?;

    println!("{} {}", mem.id, result.hits.len());
    Ok(())
}
```

Configuration falls back to env vars: `LEDGERMEM_API_KEY`, `LEDGERMEM_WORKSPACE_ID`, `LEDGERMEM_API_URL`.

## API

| Method                          | Endpoint                  |
| ------------------------------- | ------------------------- |
| `client.search`                 | `POST /v1/search`         |
| `client.memories().add`         | `POST /v1/memories`       |
| `client.memories().update`      | `PATCH /v1/memories/:id`  |
| `client.memories().delete`      | `DELETE /v1/memories/:id` |
| `client.memories().list`        | `GET /v1/memories`        |

## License

MIT
