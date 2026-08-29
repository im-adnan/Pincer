//! RPC token authentication verification
//!
//! ### Architectural Overview
//! - **What it does**: Validates security authentication tokens on incoming JSON-RPC 2.0 requests when an RPC secret is configured.
//! - **How it does**: Checks if the leading parameter in `req.params` matches `token:<rpc-secret>`; if authentication fails, constructs a standard JSON-RPC Unauthorized error response (code 1).
//! - **Where it comes from**: Called by `rpc::WebSocketHandler::handle()` before routing incoming client messages.
//! - **Where it leads to**: Grants method execution rights or halts unauthorized request dispatch.

use crate::models::{RPCError, RPCRequest, RPCResponse};

/// Validates RPC client authentication tokens against configured secrets.
pub struct RpcAuthenticator;

impl RpcAuthenticator {
    /// Validates request authentication if an RPC secret is configured.
    ///
    /// Authentication protocol:
    /// - If no secret is configured on the server, all requests are authenticated.
    /// - If a secret is configured, the client must supply `"token:<secret>"` as the first element in `params`.
    pub fn is_authenticated(req: &RPCRequest, expected_secret: Option<&str>) -> bool {
        let secret = match expected_secret {
            Some(s) if !s.is_empty() => s,
            _ => return true,
        };

        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                if !params_array.is_empty() && params_array[0].is_string() {
                    let token_str = params_array[0].as_str().unwrap();
                    if token_str == format!("token:{}", secret) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Creates an Unauthorized `RPCResponse` error payload (code 1).
    pub fn unauthorized_response(id: &str) -> RPCResponse<serde_json::Value> {
        RPCResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result: None,
            error: Some(RPCError {
                code: 1,
                message: "Unauthorized".to_string(),
            }),
        }
    }
}
