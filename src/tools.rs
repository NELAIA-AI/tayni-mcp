//! MCP Tool implementations for TAYNI

use crate::protocol::{
    InitializeResult, JsonRpcRequest, JsonRpcResponse, McpError, ServerCapabilities, ServerInfo,
    ToolCallResult, ToolDefinition, ToolsCapability,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

const VERSION: &str = "0.1.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

pub struct ToolHandler {
    compiler_path: PathBuf,
}

impl ToolHandler {
    pub fn new() -> Self {
        let compiler_path = std::env::var("TAYNI_COMPILER")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("TAYNI-c"));

        Self { compiler_path }
    }

    pub fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "initialized" => JsonRpcResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params),
            "ping" => JsonRpcResponse::success(request.id, json!({})),
            _ => JsonRpcResponse::error(request.id, McpError::method_not_found(&request.method)),
        }
    }

    fn handle_initialize(&self, id: Value) -> JsonRpcResponse {
        let result = InitializeResult {
            protocol_version: PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: "TAYNI-mcp".to_string(),
                version: VERSION.to_string(),
            },
        };
        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let tools = vec![
            ToolDefinition {
                name: "TAYNI_compile".to_string(),
                description: "Compile TAYNI source code to a native executable binary".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "TAYNI source code to compile"
                        },
                        "target": {
                            "type": "string",
                            "enum": ["windows", "linux", "macos", "macos-arm64"],
                            "default": "linux",
                            "description": "Target platform for the compiled binary"
                        },
                        "output_format": {
                            "type": "string",
                            "enum": ["base64"],
                            "default": "base64",
                            "description": "Output format for the binary (base64 encoded)"
                        }
                    },
                    "required": ["source"]
                }),
            },
            ToolDefinition {
                name: "TAYNI_check".to_string(),
                description: "Check TAYNI source code syntax without compiling".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "TAYNI source code to check"
                        }
                    },
                    "required": ["source"]
                }),
            },
            ToolDefinition {
                name: "TAYNI_info".to_string(),
                description: "Get information about TAYNI language and compiler".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "enum": ["version", "operators", "capabilities", "grammar", "examples"],
                            "description": "What information to retrieve"
                        }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "TAYNI_example".to_string(),
                description: "Get example TAYNI code for a specific task".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Description of what the program should do (e.g., 'print hello world', 'add two numbers')"
                        }
                    },
                    "required": ["task"]
                }),
            },
        ];

        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, id: Value, params: Value) -> JsonRpcResponse {
        #[derive(Deserialize)]
        struct ToolCallParams {
            name: String,
            #[serde(default)]
            arguments: Value,
        }

        let params: ToolCallParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    McpError::invalid_params(format!("Invalid tool call params: {}", e)),
                )
            }
        };

        let result = match params.name.as_str() {
            "TAYNI_compile" => self.tool_compile(params.arguments),
            "TAYNI_check" => self.tool_check(params.arguments),
            "TAYNI_info" => self.tool_info(params.arguments),
            "TAYNI_example" => self.tool_example(params.arguments),
            _ => ToolCallResult::error(format!("Unknown tool: {}", params.name)),
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn tool_compile(&self, args: Value) -> ToolCallResult {
        #[derive(Deserialize)]
        struct CompileArgs {
            source: String,
            #[serde(default = "default_target")]
            target: String,
            #[serde(default = "default_output_format")]
            #[allow(dead_code)]
            output_format: String,
        }

        fn default_target() -> String {
            "linux".to_string()
        }
        fn default_output_format() -> String {
            "base64".to_string()
        }

        let args: CompileArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResult::error(format!("Invalid arguments: {}", e)),
        };

        let temp_dir = match TempDir::new() {
            Ok(d) => d,
            Err(e) => return ToolCallResult::error(format!("Failed to create temp dir: {}", e)),
        };

        let source_path = temp_dir.path().join("input.tayni");
        let output_path = temp_dir.path().join("output");

        if let Err(e) = std::fs::File::create(&source_path)
            .and_then(|mut f| f.write_all(args.source.as_bytes()))
        {
            return ToolCallResult::error(format!("Failed to write source file: {}", e));
        }

        let target_flag = match args.target.as_str() {
            "windows" => "--emit-pe",
            "linux" => "--emit-elf",
            "macos" => "--emit-macho",
            "macos-arm64" => "--emit-macho-arm64",
            _ => return ToolCallResult::error(format!("Invalid target: {}", args.target)),
        };

        let output = Command::new(&self.compiler_path)
            .args([
                source_path.to_str().unwrap(),
                "-o",
                output_path.to_str().unwrap(),
                target_flag,
                "--json",
            ])
            .output();

        let output = match output {
            Ok(o) => o,
            Err(e) => {
                return ToolCallResult::error(format!(
                    "Failed to run compiler '{}': {}",
                    self.compiler_path.display(),
                    e
                ))
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            if let Ok(error_json) = serde_json::from_str::<Value>(&stdout) {
                return ToolCallResult::text(
                    json!({
                        "success": false,
                        "error_code": error_json.get("error_code").unwrap_or(&json!("E:UNKNOWN")),
                        "error_message": error_json.get("error_message").unwrap_or(&json!(stderr.to_string())),
                        "line": error_json.get("line"),
                        "column": error_json.get("column")
                    })
                    .to_string(),
                );
            }

            return ToolCallResult::text(
                json!({
                    "success": false,
                    "error_code": "E:COMPILE",
                    "error_message": if stderr.is_empty() { stdout.to_string() } else { stderr.to_string() }
                })
                .to_string(),
            );
        }

        // Determine the actual output file path (compiler may add extension)
        let actual_output_path = match args.target.as_str() {
            "windows" => temp_dir.path().join("output.exe"),
            _ => output_path.clone(),
        };

        let binary = match std::fs::read(&actual_output_path) {
            Ok(b) => b,
            Err(e) => {
                // Try without extension as fallback
                match std::fs::read(&output_path) {
                    Ok(b) => b,
                    Err(_) => return ToolCallResult::error(format!("Failed to read compiled binary: {}", e))
                }
            }
        };

        let binary_size = binary.len();
        let binary_base64 = base64::engine::general_purpose::STANDARD.encode(&binary);

        let format = match args.target.as_str() {
            "windows" => "PE",
            "linux" => "ELF",
            "macos" | "macos-arm64" => "Mach-O",
            _ => "unknown",
        };

        ToolCallResult::text(
            json!({
                "success": true,
                "binary_base64": binary_base64,
                "binary_size": binary_size,
                "format": format,
                "warnings": []
            })
            .to_string(),
        )
    }

    fn tool_check(&self, args: Value) -> ToolCallResult {
        #[derive(Deserialize)]
        struct CheckArgs {
            source: String,
        }

        let args: CheckArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResult::error(format!("Invalid arguments: {}", e)),
        };

        let temp_dir = match TempDir::new() {
            Ok(d) => d,
            Err(e) => return ToolCallResult::error(format!("Failed to create temp dir: {}", e)),
        };

        let source_path = temp_dir.path().join("input.tayni");

        if let Err(e) = std::fs::File::create(&source_path)
            .and_then(|mut f| f.write_all(args.source.as_bytes()))
        {
            return ToolCallResult::error(format!("Failed to write source file: {}", e));
        }

        let output = Command::new(&self.compiler_path)
            .args([source_path.to_str().unwrap(), "--check", "--json"])
            .output();

        let output = match output {
            Ok(o) => o,
            Err(e) => {
                return ToolCallResult::error(format!(
                    "Failed to run compiler '{}': {}",
                    self.compiler_path.display(),
                    e
                ))
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);

        if output.status.success() {
            if let Ok(check_result) = serde_json::from_str::<Value>(&stdout) {
                return ToolCallResult::text(
                    json!({
                        "valid": true,
                        "nodes": check_result.get("nodes").unwrap_or(&json!(0)),
                        "operators_used": check_result.get("operators_used").unwrap_or(&json!([])),
                        "capabilities_required": check_result.get("capabilities_required").unwrap_or(&json!([]))
                    })
                    .to_string(),
                );
            }

            let operators = extract_operators(&args.source);
            let nodes = count_nodes(&args.source);

            return ToolCallResult::text(
                json!({
                    "valid": true,
                    "nodes": nodes,
                    "operators_used": operators,
                    "capabilities_required": []
                })
                .to_string(),
            );
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        if let Ok(error_json) = serde_json::from_str::<Value>(&stdout) {
            return ToolCallResult::text(
                json!({
                    "valid": false,
                    "error_code": error_json.get("error_code").unwrap_or(&json!("E:SYNTAX")),
                    "error_message": error_json.get("error_message").unwrap_or(&json!(stderr.to_string())),
                    "line": error_json.get("line"),
                    "column": error_json.get("column")
                })
                .to_string(),
            );
        }

        ToolCallResult::text(
            json!({
                "valid": false,
                "error_code": "E:SYNTAX",
                "error_message": if stderr.is_empty() { stdout.to_string() } else { stderr.to_string() }
            })
            .to_string(),
        )
    }

    fn tool_info(&self, args: Value) -> ToolCallResult {
        #[derive(Deserialize)]
        struct InfoArgs {
            query: String,
        }

        let args: InfoArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResult::error(format!("Invalid arguments: {}", e)),
        };

        let result = match args.query.as_str() {
            "version" => json!({
                "language_version": "0.22",
                "compiler_version": "0.22.0",
                "mcp_server_version": VERSION
            }),
            "operators" => json!({
                "categories": {
                    "arithmetic": ["ADD", "SUB", "MUL", "DIV", "MOD", "NEG"],
                    "comparison": ["EQ", "NE", "LT", "GT", "LE", "GE"],
                    "logic": ["AND", "OR", "NOT"],
                    "memory": ["ALC", "FRE", "PUT", "GET", "CPY", "SLN"],
                    "io": ["PRT", "INP", "FOP", "FRD", "FWR", "FCL"],
                    "control": ["JMP", "JZ", "JNZ", "CALL", "RET"],
                    "network": ["TCP", "UDP", "BND", "LST", "ACC", "XMT", "RCV", "CLS"],
                    "http": ["HTTP.LISTEN", "HTTP.ACCEPT", "HTTP.RESPOND", "HTTP.GET", "HTTP.POST"],
                    "sql": ["SQL.CONNECT", "SQL.QUERY", "SQL.EXEC", "SQL.CLOSE"],
                    "json": ["JSON.PARSE", "JSON.ENCODE", "JSON.GET", "JSON.SET"]
                }
            }),
            "capabilities" => json!({
                "capabilities": {
                    "network": "Required for TCP, UDP, HTTP operations",
                    "filesystem": "Required for file operations (FOP, FRD, FWR, FCL)",
                    "sql": "Required for database operations"
                }
            }),
            "grammar" => json!({
                "syntax": {
                    "node_definition": ".name: value",
                    "operator_call": ".result: OPERATOR .arg1 .arg2",
                    "literal_string": "\"text\" or 'text'",
                    "literal_number": "42 or 3.14",
                    "comment": "# single line comment"
                },
                "examples": [
                    ".x: 10",
                    ".y: 20",
                    ".sum: ADD .x .y",
                    ".msg: \"Result: \"",
                    ".out: PRT .msg 8"
                ]
            }),
            "examples" => json!({
                "hello_world": ".msg: \"Hello World!\\n\"\n.len: 13\n.out: PRT .msg .len",
                "add_numbers": ".a: 5\n.b: 3\n.sum: ADD .a .b",
                "conditional": ".x: 10\n.zero: 0\n.cmp: GT .x .zero\n.branch: JNZ .cmp .positive"
            }),
            _ => {
                return ToolCallResult::error(format!(
                    "Unknown query: {}. Valid queries: version, operators, capabilities, grammar, examples",
                    args.query
                ))
            }
        };

        ToolCallResult::text(result.to_string())
    }

    fn tool_example(&self, args: Value) -> ToolCallResult {
        #[derive(Deserialize)]
        struct ExampleArgs {
            task: String,
        }

        let args: ExampleArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return ToolCallResult::error(format!("Invalid arguments: {}", e)),
        };

        let task_lower = args.task.to_lowercase();

        let (source, description, operators) = if task_lower.contains("hello")
            || task_lower.contains("print")
            || task_lower.contains("output")
        {
            (
                r#".msg: "Hello World!\n"
.len: 13
.out: PRT .msg .len"#,
                "Prints 'Hello World!' followed by a newline to stdout",
                vec!["PRT"],
            )
        } else if task_lower.contains("add") || task_lower.contains("sum") {
            (
                r#".a: 5
.b: 3
.sum: ADD .a .b
# Result is stored in .sum (value: 8)"#,
                "Adds two numbers together",
                vec!["ADD"],
            )
        } else if task_lower.contains("subtract") || task_lower.contains("minus") {
            (
                r#".a: 10
.b: 3
.diff: SUB .a .b
# Result is stored in .diff (value: 7)"#,
                "Subtracts one number from another",
                vec!["SUB"],
            )
        } else if task_lower.contains("multiply") || task_lower.contains("product") {
            (
                r#".a: 6
.b: 7
.product: MUL .a .b
# Result is stored in .product (value: 42)"#,
                "Multiplies two numbers",
                vec!["MUL"],
            )
        } else if task_lower.contains("divide") || task_lower.contains("division") {
            (
                r#".a: 20
.b: 4
.quotient: DIV .a .b
# Result is stored in .quotient (value: 5)"#,
                "Divides one number by another",
                vec!["DIV"],
            )
        } else if task_lower.contains("compare") || task_lower.contains("greater") || task_lower.contains("less") {
            (
                r#".x: 10
.y: 5
.is_greater: GT .x .y
.is_less: LT .x .y
.is_equal: EQ .x .y
# .is_greater = 1 (true), .is_less = 0 (false), .is_equal = 0 (false)"#,
                "Compares two numbers using various comparison operators",
                vec!["GT", "LT", "EQ"],
            )
        } else if task_lower.contains("loop") || task_lower.contains("iterate") || task_lower.contains("repeat") {
            (
                r#"# Loop that counts from 1 to 5
.counter: 1
.limit: 5
.one: 1

@loop_start:
.msg: "Count: "
.out1: PRT .msg 7
# Print counter value here
.counter: ADD .counter .one
.cmp: LE .counter .limit
.branch: JNZ .cmp @loop_start"#,
                "A simple counting loop from 1 to 5",
                vec!["ADD", "LE", "JNZ", "PRT"],
            )
        } else if task_lower.contains("file") || task_lower.contains("read") || task_lower.contains("write") {
            (
                r#"# Write to a file
.filename: "output.txt"
.fnamelen: 10
.mode: "w"
.modelen: 1
.fd: FOP .filename .fnamelen .mode .modelen

.content: "Hello from TAYNI!\n"
.contentlen: 20
.written: FWR .fd .content .contentlen

.close: FCL .fd"#,
                "Opens a file, writes content to it, and closes it",
                vec!["FOP", "FWR", "FCL"],
            )
        } else if task_lower.contains("http") || task_lower.contains("server") || task_lower.contains("web") {
            (
                r#"# Simple HTTP server
.port: 8080
.server: HTTP.LISTEN .port

@accept_loop:
.conn: HTTP.ACCEPT .server
.response: "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello World!"
.resplen: 56
.sent: HTTP.RESPOND .conn .response .resplen
.branch: JMP @accept_loop"#,
                "A minimal HTTP server that responds with 'Hello World!'",
                vec!["HTTP.LISTEN", "HTTP.ACCEPT", "HTTP.RESPOND", "JMP"],
            )
        } else if task_lower.contains("memory") || task_lower.contains("allocate") || task_lower.contains("buffer") {
            (
                r#"# Allocate and use memory
.size: 1024
.buffer: ALC .size

# Store a value at offset 0
.offset: 0
.value: 42
.stored: PUT .buffer .offset .value

# Read it back
.retrieved: GET .buffer .offset

# Free the memory
.freed: FRE .buffer"#,
                "Allocates memory, stores a value, retrieves it, and frees the memory",
                vec!["ALC", "PUT", "GET", "FRE"],
            )
        } else {
            (
                r#"# Basic TAYNI program template
.msg: "TAYNI Program\n"
.len: 15
.out: PRT .msg .len

# Add your logic here
.x: 0
.y: 0"#,
                "A basic program template - customize for your specific task",
                vec!["PRT"],
            )
        };

        ToolCallResult::text(
            json!({
                "source": source,
                "description": description,
                "operators_used": operators,
                "binary_size_estimate": "~2-4KB"
            })
            .to_string(),
        )
    }
}

fn extract_operators(source: &str) -> Vec<String> {
    let known_operators = [
        "ADD", "SUB", "MUL", "DIV", "MOD", "NEG", "EQ", "NE", "LT", "GT", "LE", "GE", "AND", "OR",
        "NOT", "ALC", "FRE", "PUT", "GET", "CPY", "SLN", "PRT", "INP", "FOP", "FRD", "FWR", "FCL",
        "JMP", "JZ", "JNZ", "CALL", "RET", "TCP", "UDP", "BND", "LST", "ACC", "XMT", "RCV", "CLS",
        "HTTP.LISTEN", "HTTP.ACCEPT", "HTTP.RESPOND", "HTTP.GET", "HTTP.POST", "SQL.CONNECT",
        "SQL.QUERY", "SQL.EXEC", "SQL.CLOSE", "JSON.PARSE", "JSON.ENCODE", "JSON.GET", "JSON.SET",
    ];

    let mut found = Vec::new();
    for op in &known_operators {
        if source.contains(op) {
            found.push(op.to_string());
        }
    }
    found
}

fn count_nodes(source: &str) -> usize {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed.starts_with('.')
        })
        .count()
}
