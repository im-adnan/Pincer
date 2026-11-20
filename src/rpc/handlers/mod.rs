//! Re-exports handler functions
//!
//! ### Architectural Overview
//! - **What it does**: Exposes granular RPC handler groups (task control, task lifecycle, queries, options, introspection, resolution, session, and system reflection).
//! - **How it does**: Declares handler submodules and re-exports their respective structs (`TaskControlHandlers`, `TaskLifecycleHandlers`, `TaskQueryHandlers`, etc.).
//! - **Where it comes from**: Imported by `rpc::MethodRouter`.
//! - **Where it leads to**: Aggregates handler logic separating transport parsing from domain manager business logic.

pub mod add_metalink;
pub mod add_torrent;
pub mod add_uri;
pub mod introspection;
pub mod options;
pub mod resolve;
pub mod session;
pub mod system;
pub mod task_control;
pub mod task_lifecycle;
pub mod task_query;

// Re-export handler structs
pub use add_metalink::AddMetalinkHandler;
pub use add_torrent::AddTorrentHandler;
pub use add_uri::AddUriHandler;
pub use introspection::IntrospectionHandlers;
pub use options::OptionsRpcHandlers;
pub use resolve::ResolveHandlers;
pub use session::SessionRpcHandlers;
pub use system::SystemRpcHandlers;
pub use task_control::TaskControlHandlers;
pub use task_lifecycle::TaskLifecycleHandlers;
pub use task_query::TaskQueryHandlers;

/// Extracts HTTP headers from JSON-RPC options map.
/// Supports "header" (as string or array of strings), "user-agent" / "userAgent",
/// "referer", "cookie", and "authorization".
pub fn extract_headers(options: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut headers = Vec::new();

    if let Some(header_val) = options.get("header") {
        if let Some(header_str) = header_val.as_str() {
            for line in header_str.split('\n') {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    headers.push(trimmed.to_string());
                }
            }
        } else if let Some(header_arr) = header_val.as_array() {
            for item in header_arr {
                if let Some(s) = item.as_str() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        headers.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    if let Some(ua) = options
        .get("user-agent")
        .or_else(|| options.get("userAgent"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        && !headers
            .iter()
            .any(|h| h.to_lowercase().starts_with("user-agent:"))
    {
        headers.push(format!("User-Agent: {}", ua.trim()));
    }

    if let Some(referer) = options
        .get("referer")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        && !headers
            .iter()
            .any(|h| h.to_lowercase().starts_with("referer:"))
    {
        headers.push(format!("Referer: {}", referer.trim()));
    }

    if let Some(cookie) = options
        .get("cookie")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        && !headers
            .iter()
            .any(|h| h.to_lowercase().starts_with("cookie:"))
    {
        headers.push(format!("Cookie: {}", cookie.trim()));
    }

    if let Some(auth) = options
        .get("authorization")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        && !headers
            .iter()
            .any(|h| h.to_lowercase().starts_with("authorization:"))
    {
        headers.push(format!("Authorization: {}", auth.trim()));
    }

    headers
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_headers_from_string_and_options() {
        let options_json = json!({
            "header": "Accept: text/html\nAccept-Language: en-US",
            "user-agent": "CustomAgent/1.0",
            "referer": "https://example.com",
            "cookie": "session=abc",
            "authorization": "Bearer xyz"
        });
        let options = options_json.as_object().unwrap();
        let headers = extract_headers(options);

        assert!(headers.contains(&"Accept: text/html".to_string()));
        assert!(headers.contains(&"Accept-Language: en-US".to_string()));
        assert!(headers.contains(&"User-Agent: CustomAgent/1.0".to_string()));
        assert!(headers.contains(&"Referer: https://example.com".to_string()));
        assert!(headers.contains(&"Cookie: session=abc".to_string()));
        assert!(!headers.contains(&"Authorization: Bearx xyz".to_string()));
        assert!(headers.contains(&"Authorization: Bearer xyz".to_string()));
    }

    #[test]
    fn test_extract_headers_from_array() {
        let options_json = json!({
            "header": [
                "Accept: text/html,application/xhtml+xml",
                "Sec-Fetch-Mode: navigate"
            ],
            "user-agent": "Chrome/131.0.0.0",
            "referer": "https://example.com/"
        });
        let options = options_json.as_object().unwrap();
        let headers = extract_headers(options);

        assert_eq!(headers.len(), 4);
        assert!(headers.contains(&"Accept: text/html,application/xhtml+xml".to_string()));
        assert!(headers.contains(&"Sec-Fetch-Mode: navigate".to_string()));
        assert!(headers.contains(&"User-Agent: Chrome/131.0.0.0".to_string()));
        assert!(headers.contains(&"Referer: https://example.com/".to_string()));
    }

    #[test]
    fn test_extract_headers_no_duplicate_user_agent() {
        let options_json = json!({
            "header": [
                "User-Agent: ExistingUA/2.0"
            ],
            "user-agent": "IgnoredUA/1.0"
        });
        let options = options_json.as_object().unwrap();
        let headers = extract_headers(options);

        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], "User-Agent: ExistingUA/2.0");
    }
}
