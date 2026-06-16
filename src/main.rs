//! TAYNI MCP Server
//!
//! Model Context Protocol server that enables AI agents to compile TAYNI code.
//! Communicates via JSON-RPC over stdin/stdout.

pub mod protocol;
pub mod tools;

use protocol::{JsonRpcRequest, JsonRpcResponse, McpError};
use std::io::{self, BufRead, Write};
use tools::ToolHandler;

fn main() {
    let handler = ToolHandler::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => handler.handle_request(request),
            Err(e) => JsonRpcResponse::error(
                serde_json::Value::Null,
                McpError::parse_error(format!("Invalid JSON: {}", e)),
            ),
        };

        let response_json = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id":null}"#
                .to_string()
        });

        if writeln!(stdout, "{}", response_json).is_err() {
            break;
        }
        let _ = stdout.flush();
    }
}
