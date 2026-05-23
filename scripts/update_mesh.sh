#!/usr/bin/env bash
# Mesh-wide update orchestrator.
# Triggers an update on all connected P2P nodes via the mesh MCP system.
# Each node will download the correct binary for its platform and restart.
#
# Usage: ./update_mesh.sh [--repo user/repo] [--port 3000]

set -euo pipefail

REPO=""
PORT=3000

while [ $# -gt 0 ]; do
    case "$1" in
        --repo) REPO="$2"; shift 2 ;;
        --port) PORT="$2"; shift 2 ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

P2P_URL="http://127.0.0.1:$PORT"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -z "$REPO" ]; then
    if git -C "$PROJECT_DIR" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
        REMOTE_URL="$(git -C "$PROJECT_DIR" remote get-url origin 2>/dev/null || true)"
        REPO="$(echo "$REMOTE_URL" | sed 's/.*github.com[:\/]//; s/\.git$//')"
    fi
fi

if [ -z "$REPO" ]; then
    echo "Error: Could not detect repo. Provide --repo <user/repo>"
    exit 1
fi

echo "=== P2P Mesh Update ==="
echo "  Repo:   $REPO"
echo "  P2P:    $P2P_URL"
echo ""

# 1. Update local node first
echo "--- Updating local node ---"
curl -s -X POST "$P2P_URL/api/update" > /dev/null 2>&1 && echo "  Local update triggered" || echo "  Local update failed (node may be running old version without /api/update)"
echo ""

# 2. Broadcast update to all connected peers via mesh MCP query
echo "--- Broadcasting update to mesh ---"
REQ_ID="mesh-update-$(date +%s)-$$"
RESPONSE=$(curl -s -X POST "$P2P_URL/api/mcp/query" \
  -H "Content-Type: application/json" \
  -d "{\"request_id\":\"$REQ_ID\",\"hop_count\":2,\"tool_name\":\"update\",\"arguments\":{}}" 2>/dev/null)

if echo "$RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('local',{}).get('success',False))" 2>/dev/null | grep -q True; then
    echo "  Update broadcast sent to mesh (hop_count=2)"
else
    echo "  Mesh broadcast returned: $(echo "$RESPONSE" | head -c 200)"
fi

echo ""
echo "Done. Nodes should restart with the updated binary within seconds."
