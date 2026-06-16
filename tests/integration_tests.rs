//! Tests for TAYNI MCP Server

#[path = "../src/protocol.rs"]
mod protocol;

#[path = "../src/tools.rs"]
mod tools;

use serde_json::{json, Value};

mod common {
    use super::*;

    pub fn send_request(method: &str, params: Value) -> Value {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let handler = tools::ToolHandler::new();
        let req: protocol::JsonRpcRequest = serde_json::from_value(request).unwrap();
        let response = handler.handle_request(req);
        serde_json::to_value(response).unwrap()
    }
}

#[test]
fn test_initialize() {
    let response = common::send_request("initialize", json!({}));

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "TAYNI-mcp");
}

#[test]
fn test_tools_list() {
    let response = common::send_request("tools/list", json!({}));

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    let tools = result["tools"].as_array().unwrap();

    assert_eq!(tools.len(), 4);

    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();

    assert!(tool_names.contains(&"TAYNI_compile"));
    assert!(tool_names.contains(&"TAYNI_check"));
    assert!(tool_names.contains(&"TAYNI_info"));
    assert!(tool_names.contains(&"TAYNI_example"));
}

#[test]
fn test_TAYNI_info_version() {
    let response = common::send_request(
        "tools/call",
        json!({
            "name": "TAYNI_info",
            "arguments": {
                "query": "version"
            }
        }),
    );

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let info: Value = serde_json::from_str(content).unwrap();

    assert_eq!(info["language_version"], "0.22");
}

#[test]
fn test_TAYNI_info_operators() {
    let response = common::send_request(
        "tools/call",
        json!({
            "name": "TAYNI_info",
            "arguments": {
                "query": "operators"
            }
        }),
    );

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let info: Value = serde_json::from_str(content).unwrap();

    assert!(info["categories"]["arithmetic"].as_array().is_some());
    assert!(info["categories"]["io"].as_array().is_some());
}

#[test]
fn test_TAYNI_example_hello_world() {
    let response = common::send_request(
        "tools/call",
        json!({
            "name": "TAYNI_example",
            "arguments": {
                "task": "print hello world"
            }
        }),
    );

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let example: Value = serde_json::from_str(content).unwrap();

    assert!(example["source"].as_str().unwrap().contains("Hello World"));
    assert!(example["operators_used"]
        .as_array()
        .unwrap()
        .contains(&json!("PRT")));
}

#[test]
fn test_TAYNI_example_add_numbers() {
    let response = common::send_request(
        "tools/call",
        json!({
            "name": "TAYNI_example",
            "arguments": {
                "task": "add two numbers"
            }
        }),
    );

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let example: Value = serde_json::from_str(content).unwrap();

    assert!(example["source"].as_str().unwrap().contains("ADD"));
    assert!(example["operators_used"]
        .as_array()
        .unwrap()
        .contains(&json!("ADD")));
}

#[test]
fn test_unknown_method() {
    let response = common::send_request("unknown/method", json!({}));

    assert!(response.get("error").is_some());
    let error = response.get("error").unwrap();
    assert_eq!(error["code"], -32601);
}

#[test]
fn test_unknown_tool() {
    let response = common::send_request(
        "tools/call",
        json!({
            "name": "unknown_tool",
            "arguments": {}
        }),
    );

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    assert_eq!(result["isError"], true);
}

#[test]
fn test_ping() {
    let response = common::send_request("ping", json!({}));
    assert!(response.get("result").is_some());
}
