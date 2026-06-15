# NELAIA MCP Server

MCP (Model Context Protocol) server that enables AI agents to compile NELAIA code to native executables.

## Installation

### From Source

```bash
cd nelaia-mcp
cargo build --release
```

The binary will be at `target/release/nelaia-mcp` (or `nelaia-mcp.exe` on Windows).

### Prerequisites

- The `nelaia-c` compiler must be installed and available in PATH
- Or set the `NELAIA_COMPILER` environment variable to the compiler path

## Usage with Claude Desktop

Add to your Claude Desktop configuration (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "nelaia": {
      "command": "nelaia-mcp",
      "args": []
    }
  }
}
```

Or with a custom compiler path:

```json
{
  "mcpServers": {
    "nelaia": {
      "command": "nelaia-mcp",
      "args": [],
      "env": {
        "NELAIA_COMPILER": "/path/to/nelaia-c"
      }
    }
  }
}
```

## Available Tools

### `nelaia_compile`

Compile NELAIA source code to a native executable.

**Input:**
- `source` (string, required): NELAIA source code
- `target` (string, optional): Target platform - `windows`, `linux`, `macos`, `macos-arm64` (default: `linux`)
- `output_format` (string, optional): Output format - `base64` (default: `base64`)

**Output:**
```json
{
  "success": true,
  "binary_base64": "TVqQAAMAAAA...",
  "binary_size": 2048,
  "format": "ELF",
  "warnings": []
}
```

### `nelaia_check`

Check NELAIA source code syntax without compiling.

**Input:**
- `source` (string, required): NELAIA source code to check

**Output:**
```json
{
  "valid": true,
  "nodes": 5,
  "operators_used": ["PRT", "ADD"],
  "capabilities_required": []
}
```

### `nelaia_info`

Get information about NELAIA language and compiler.

**Input:**
- `query` (string, required): One of `version`, `operators`, `capabilities`, `grammar`, `examples`

**Output:** Structured information based on query type.

### `nelaia_example`

Get example NELAIA code for a specific task.

**Input:**
- `task` (string, required): Description of what the program should do

**Output:**
```json
{
  "source": ".msg: \"Hello World!\\n\"\n.len: 13\n.out: PRT .msg .len",
  "description": "Prints 'Hello World!' to stdout",
  "operators_used": ["PRT"],
  "binary_size_estimate": "~2KB"
}
```

## Protocol

The server communicates via JSON-RPC 2.0 over stdin/stdout, following the MCP specification.

### Supported Methods

- `initialize` - Initialize the MCP session
- `initialized` - Notification that initialization is complete
- `tools/list` - List available tools
- `tools/call` - Call a tool
- `ping` - Health check

## Example Session

```bash
# Start the server
echo '{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}' | nelaia-mcp

# List tools
echo '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}' | nelaia-mcp

# Compile code
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"nelaia_compile","arguments":{"source":".msg: \"Hi\"\\n.out: PRT .msg 2"}},"id":3}' | nelaia-mcp
```

## Development

```bash
# Run in development
cargo run

# Run tests
cargo test

# Build release
cargo build --release
```

## License

MIT
