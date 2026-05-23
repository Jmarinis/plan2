#!/usr/bin/env bash
# Mesh-wide update orchestrator.
# Triggers update.sh on all connected P2P nodes via the mesh MCP system.
#
# Prerequisites:
#   1. update.sh must be present at --script-path on each node (default: ./update.sh)
#   2. The local P2P node must be running on --port (default: 3000)
#
# Usage: ./update_mesh.sh [--repo user/repo] [--port 3000] [--script-path /path/to/update.sh]

set -euo pipefail

REPO=""
PORT=3000
SCRIPT_PATH=""

# Parse args
while [ $# -gt 0 ]; do
    case "$1" in
        --repo) REPO="$2"; shift 2 ;;
        --port) PORT="$2"; shift 2 ;;
        --script-path) SCRIPT_PATH="$2"; shift 2 ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

P2P_URL="http://127.0.0.1:$PORT"

# Auto-detect repo
if [ -z "$REPO" ]; then
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
    if git -C "$PROJECT_DIR" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
        REMOTE_URL="$(git -C "$PROJECT_DIR" remote get-url origin 2>/dev/null || true)"
        REPO="$(echo "$REMOTE_URL" | sed 's/.*github.com[:\/]//; s/\.git$//')"
    fi
fi

if [ -z "$REPO" ]; then
    echo "Error: Could not detect repo. Provide --repo <user/repo>"
    exit 1
fi

if [ -z "$SCRIPT_PATH" ]; then
    SCRIPT_PATH="$(cd "$(dirname "$0")" && pwd)/update.sh"
fi

echo "=== P2P Mesh Update ==="
echo "  Repo:   $REPO"
echo "  P2P:    $P2P_URL"
echo "  Script: $SCRIPT_PATH"
echo ""

# 1. Update local node first
echo "--- Updating local node ---"
if [ -f "$SCRIPT_PATH" ]; then
    bash "$SCRIPT_PATH" "$REPO" "$PORT"
    echo ""
    # Wait for local node to come back up
    sleep 3
else
    echo "  update.sh not found at $SCRIPT_PATH (local update skipped)"
fi

# 2. Get list of connected peers from the mesh
echo "--- Fetching mesh topology ---"
STATUS="$(curl -sf "$P2P_URL/api/status" 2>/dev/null)" || {
    echo "  Local node not responding at $P2P_URL"
    exit 1
}

CONNECTED="$(echo "$STATUS" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for p in d.get('connected_peers', []):
    if p.get('connected'):
        print(f'{p[\"address\"]}:{p[\"port\"]} {p.get(\"hostname\", \"?\")}')
" 2>/dev/null)" || true

if [ -z "$CONNECTED" ]; then
    echo "  No connected peers. Local node updated."
    exit 0
fi

echo ""
echo "--- Triggering update on connected peers ---"
while IFS= read -r line; do
    [ -z "$line" ] && continue
    PEER_ADDR="$(echo "$line" | cut -d' ' -f1)"
    PEER_NAME="$(echo "$line" | cut -d' ' -f2)"
    PEER_IP="$(echo "$PEER_ADDR" | cut -d: -f1)"
    PEER_PORT="$(echo "$PEER_ADDR" | cut -d: -f2)"

    echo "  [$PEER_NAME] $PEER_ADDR"

    # Check if the peer has the update script accessible via HTTP
    # If the peer exposes a file server, we could upload it. Instead,
    # we use the mesh MCP query to run a command on the peer.
    #
    # Option A: Trigger via MCP query (needs an "update" tool on the node)
    # curl -s -X POST "$P2P_URL/api/mcp/query" \
    #   -H "Content-Type: application/json" \
    #   -d "{\"request_id\":\"...\",\"hop_count\":1,\"tool_name\":\"update\",\"arguments\":{\"repo\":\"$REPO\"}}"
    #
    # Option B: SSH into the peer (requires SSH access)
    # ssh "$PEER_NAME" "bash -s" < "$SCRIPT_PATH" -- "$REPO" "$PEER_PORT"
    #
    # Option C: HTTP endpoint on the peer itself
    echo "    Triggering update via SSH..."
    if ssh -o ConnectTimeout=5 -o BatchMode=yes "$PEER_NAME" "bash -s" -- "$REPO" "$PEER_PORT" < "$SCRIPT_PATH" 2>/dev/null; then
        echo "    ✅ $PEER_NAME updated"
    else
        echo "    ⚠️  SSH failed. To update manually:"
        echo "       ssh $PEER_NAME 'bash -s' -- $REPO $PEER_PORT < $SCRIPT_PATH"
        echo "       or copy and run update.sh on $PEER_NAME"
    fi
    echo ""
done <<< "$CONNECTED"
