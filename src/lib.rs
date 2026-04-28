//! Official Rust SDK for the LedgerMem API.

use std::collections::HashMap;
use std::env;
use std::time::Duration;

use reqwest::{header, Client as HttpClient, Method, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.proofly.dev";
const USER_AGENT: &str = concat!("ledgermem-rust/", env!("CARGO_PKG_VERSION"));

/// Configuration for [`Client`].
#[derive(Debug, Default, Clone)]
pub struct ClientConfig {
    pub api_key: Option<String>,
    pub workspace_id: Option<String>,
    pub base_url: Option<String>,
    pub timeout: Option<Duration>,
}

/// Errors returned by the SDK.
#[derive(Debug, Error)]
pub enum Error {
    #[error("ledgermem: http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("ledgermem: decode error: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("ledgermem: api error: {status} {message}")]
    Api { status: u16, message: String, body: String },
}

/// API result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// LedgerMem API client.
#[derive(Debug, Clone)]
pub struct Client {
    http: HttpClient,
    base_url: String,
}

impl Client {
    /// Build a new client. Falls back to env vars for any missing config.
    pub fn new(cfg: ClientConfig) -> Result<Self> {
        let api_key = cfg.api_key.or_else(|| env::var("LEDGERMEM_API_KEY").ok());
        let workspace_id = cfg
            .workspace_id
            .or_else(|| env::var("LEDGERMEM_WORKSPACE_ID").ok());
        let base_url = cfg
            .base_url
            .or_else(|| env::var("LEDGERMEM_API_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let timeout = cfg.timeout.unwrap_or_else(|| Duration::from_secs(30));

        let mut headers = header::HeaderMap::new();
        headers.insert(header::ACCEPT, header::HeaderValue::from_static("application/json"));
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(USER_AGENT),
        );
        if let Some(key) = api_key.as_deref() {
            let mut v = header::HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|e| Error::Api { status: 0, message: e.to_string(), body: String::new() })?;
            v.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, v);
        }
        if let Some(ws) = workspace_id.as_deref() {
            headers.insert(
                "x-workspace-id",
                header::HeaderValue::from_str(ws).map_err(|e| Error::Api {
                    status: 0,
                    message: e.to_string(),
                    body: String::new(),
                })?,
            );
        }

        let http = HttpClient::builder()
            .default_headers(headers)
            .timeout(timeout)
            .build()?;

        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Access the memories sub-resource.
    pub fn memories(&self) -> Memories<'_> {
        Memories { client: self }
    }

    /// Run a semantic search.
    pub async fn search(&self, input: SearchInput) -> Result<SearchResult> {
        self.request(Method::POST, "/v1/search", &[], Some(&input)).await
    }

    async fn request<T, B>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&B>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + Default,
        B: Serialize + ?Sized,
    {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.http.request(method, &url);
        if !query.is_empty() {
            req = req.query(query);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().await?;
        let status = resp.status();
        if status == StatusCode::NO_CONTENT {
            return Ok(T::default());
        }
        let bytes = resp.bytes().await?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes).into_owned();
            let message = serde_json::from_slice::<ApiErrorBody>(&bytes)
                .ok()
                .and_then(|b| b.message.or(b.error))
                .unwrap_or_default();
            return Err(Error::Api { status: status.as_u16(), message, body });
        }
        if bytes.is_empty() {
            return Ok(T::default());
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: Option<String>,
    error: Option<String>,
}

/// Memories sub-client.
pub struct Memories<'a> {
    client: &'a Client,
}

impl Memories<'_> {
    pub async fn add(&self, input: AddMemoryInput) -> Result<Memory> {
        self.client
            .request(Method::POST, "/v1/memories", &[], Some(&input))
            .await
    }

    pub async fn update(&self, id: &str, input: UpdateMemoryInput) -> Result<Memory> {
        let path = format!("/v1/memories/{}", encode_path_segment(id));
        self.client.request(Method::PATCH, &path, &[], Some(&input)).await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let path = format!("/v1/memories/{}", encode_path_segment(id));
        let _: Empty = self.client.request::<Empty, ()>(Method::DELETE, &path, &[], None).await?;
        Ok(())
    }

    pub async fn list(&self, input: ListMemoriesInput) -> Result<ListMemoriesResult> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = input.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(cursor) = input.cursor {
            query.push(("cursor", cursor));
        }
        if let Some(actor) = input.actor_id {
            query.push(("actorId", actor));
        }
        self.client
            .request::<ListMemoriesResult, ()>(Method::GET, "/v1/memories", &query, None)
            .await
    }
}

#[derive(Debug, Default, Deserialize)]
struct Empty {}

fn encode_path_segment(s: &str) -> String {
    // Percent-encode characters that would break a path segment. We avoid
    // pulling in `percent-encoding` to keep the dependency surface small.
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let c = *b;
        let unreserved = c.is_ascii_alphanumeric()
            || matches!(c, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}

/// A single stored memory.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// One entry in a search result.
#[derive(Debug, Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub content: String,
    pub score: f64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SearchResult {
    #[serde(default)]
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMemoryInput {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemoryInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Default)]
pub struct ListMemoriesInput {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub actor_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMemoriesResult {
    #[serde(default)]
    pub data: Vec<Memory>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}
