# XPipe MCP SSH Host Access

Use this skill to gather information from SSH hosts configured in XPipe by
accessing its local MCP server over SSE/streamable HTTP transport.

## Connection Details

| Field | Value |
|---|---|
| **URL** | `http://localhost:21721/mcp` |
| **Transport** | streamable HTTP (SSE) |
| **API Key** | `889a13b0-bcdc-405b-bba0-20ae04e2972b` |
| **Auth Header** | `Authorization: Bearer <apiKey>` |
| **Session** | POST initialize → extract `Mcp-Session-Id` → use in `Mcp-Session-Id` header |

All requests require:
- `Content-Type: application/json`
- `Accept: application/json, text/event-stream`
- `Authorization: Bearer 889a13b0-bcdc-405b-bba0-20ae04e2972b`
- `Mcp-Session-Id: <sessionId>` (after initialization)

## Available Tools

| Tool | Purpose | Read-only |
|---|---|---|
| `list_systems` | List all configured hosts | Yes |
| `run_command` | Execute shell command on host | No |
| `read_file` | Read file contents | Yes |
| `list_files` | List directory contents | Yes |
| `find_file` | Search for files | Yes |
| `get_file_info` | Get file metadata | Yes |
| `write_file` | Write text to file | No |
| `create_file` | Create a new file | No |
| `create_directory` | Create a directory | No |
| `open_terminal` | Open terminal window | No |
| `run_script` | Run predefined script | No |
| `toggle_state` | Start/stop tunnels/services | No |
| `call_api` | Raw XPipe HTTP API call | No |

## Workflow

### 1. Initialize Session
```bash
curl -s -X POST "http://localhost:21721/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "Authorization: Bearer 889a13b0-bcdc-405b-bba0-20ae04e2972b" \
  -d '{"jsonrpc":"2.0","method":"initialize",
       "params":{"protocolVersion":"2025-03-26","capabilities":{},
        "clientInfo":{"name":"agent","version":"1.0"}},"id":1}'
```
Extract `Mcp-Session-Id` from response headers.

### 2. List Available Hosts
```bash
SESSION_ID="..."
curl -s -X POST "http://localhost:21721/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "Authorization: Bearer 889a13b0-bcdc-405b-bba0-20ae04e2972b" \
  -H "Mcp-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","method":"tools/call",
       "params":{"name":"list_systems","arguments":{"filter":"**"}},"id":2}' \
  | grep "^data:" | sed 's/^data: //'
```

### 3. Run Commands on a Host
```bash
curl -s -X POST "http://localhost:21721/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "Authorization: Bearer 889a13b0-bcdc-405b-bba0-20ae04e2972b" \
  -H "Mcp-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","method":"tools/call",
       "params":{"name":"run_command",
        "arguments":{"system":"<host-path>","command":"<command>"}},"id":3}' \
  | grep "^data:" | sed 's/^data: //'
```

### 4. Read a File
```bash
curl -s -X POST "http://localhost:21721/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "Authorization: Bearer 889a13b0-bcdc-405b-bba0-20ae04e2972b" \
  -H "Mcp-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","method":"tools/call",
       "params":{"name":"read_file",
        "arguments":{"system":"<host-path>","path":"<file-path>"}},"id":4}'
```

## Configured Hosts (from XPipe vault)

| System Path | OS |
|---|---|
| `jmarinis@jims-m1-mini.local` | macOS 26.5 |
| `jmarinis@jims-macbook-air.local` | EndeavourOS |
| `jmarinis@linux-mint.local` | Linux Mint 22.3 |
| `jmarinis@debian.local` | Debian 13 (trixie) |
| `jmarinis@macpro.local` | Debian 13 (trixie) |
| `jmarinis@c740.local` | EndeavourOS |
| `jmarinis@node01.local` | Debian 12 (bookworm) |
| `jmarinis@node02.local` | Debian 12 (bookworm) |
| `jmarinis@node03.local` | Debian 12 (bookworm) |
| `jmarinis@node04.local` | Debian 12 (bookworm) |
| `jmarinis@orin.local` | Ubuntu 22.04 |
| `jmarinis@main.siniram.com` | Debian 13 (trixie) |
| `jmarinis@apollo.local` | Debian (password) |
| `jmarinis@internal` | (password) |
| `Local Machine` | macOS 26.5 Tahoe |

## Claude Desktop Configuration

Add this to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "xpipe": {
      "type": "streamable-http",
      "url": "http://localhost:21721/mcp",
      "headers": {
        "Authorization": "Bearer 889a13b0-bcdc-405b-bba0-20ae04e2972b"
      }
    }
  }
}
```

## Notes

- The MCP session expires; re-initialize if you get auth errors.
- `run_command` is a mutation tool — it executes arbitrary shell commands on target hosts.
- `read_file` and `run_command` require the host system identifier (path from `list_systems`).
- For information-gathering tasks, prefer reading files and running non-destructive commands
  (`uname`, `uptime`, `df`, `free`, `ps`, `journalctl`, etc).
- Responses come as SSE (`data: {...}`), extract with `grep "^data:" | sed 's/^data: //'`.
