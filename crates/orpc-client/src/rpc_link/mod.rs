#[cfg(target_arch = "wasm32")]
mod fetch;
#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(target_arch = "wasm32")]
pub use fetch::FetchError;

/// HTTP transport for the oRPC RPC protocol.
///
/// Converts dot-separated procedure paths to URL paths and sends
/// HTTP POST requests matching the `@orpc/client` wire format.
///
/// # Example
/// ```ignore
/// let link = RpcLink::new("http://localhost:3000/rpc");
/// let client = Client::with_link(link);
/// ```
pub struct RpcLink {
    #[cfg(not(target_arch = "wasm32"))]
    http: reqwest::Client,
    base_url: String,
}

impl RpcLink {
    /// Create a new RPC link with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        RpcLink {
            #[cfg(not(target_arch = "wasm32"))]
            http: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Use a custom `reqwest::Client` (for proxy, TLS, auth, etc.).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.http = client;
        self
    }

    fn build_url(&self, path: &str) -> String {
        let path_url = path.replace('.', "/");
        let base = self.base_url.trim_end_matches('/');
        format!("{base}/{path_url}")
    }

    fn build_body(input: serde_json::Value) -> serde_json::Value {
        if input.is_null() {
            // No input: send {} (matches @orpc/client behavior)
            serde_json::json!({})
        } else {
            serde_json::json!({ "json": input })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_simple() {
        let link = RpcLink::new("http://localhost:3000/rpc");
        assert_eq!(link.build_url("ping"), "http://localhost:3000/rpc/ping");
    }

    #[test]
    fn build_url_nested() {
        let link = RpcLink::new("http://localhost:3000/rpc");
        assert_eq!(
            link.build_url("planet.find"),
            "http://localhost:3000/rpc/planet/find"
        );
    }

    #[test]
    fn build_url_trailing_slash() {
        let link = RpcLink::new("http://localhost:3000/rpc/");
        assert_eq!(link.build_url("ping"), "http://localhost:3000/rpc/ping");
    }

    #[test]
    fn build_body_null_input() {
        let body = RpcLink::build_body(serde_json::Value::Null);
        assert_eq!(body, serde_json::json!({}));
    }

    #[test]
    fn build_body_with_input() {
        let body = RpcLink::build_body(serde_json::json!("Earth"));
        assert_eq!(body, serde_json::json!({"json": "Earth"}));
    }

    #[test]
    fn build_body_with_object() {
        let body = RpcLink::build_body(serde_json::json!({"name": "Earth"}));
        assert_eq!(body, serde_json::json!({"json": {"name": "Earth"}}));
    }
}
