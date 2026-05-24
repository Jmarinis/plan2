#!/usr/bin/env bash
# Mesh orchestrator: inspect, start, and upgrade P2P nodes across the mesh via SSH.
#
# Usage: ./meshctl.sh [options]
#   --port  PORT     Local P2P node port (default: 3000)
#   --user  USER     SSH user (default: current user)
#   --repo  REPO     GitHub user/repo (default: auto-detect from git remote)
#   --path  PATH     Project path on remote machines (default: same as local)
#   --dry-run        Print actions without executing

set -euo pipefail

PORT=3000
SSH_USER=""
REPO=""
REMOTE_PATH=""
DRY_RUN=false
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

while [ $# -gt 0 ]; do
    case "$1" in
        --port) PORT="$2"; shift 2 ;;
        --user) SSH_USER="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        --path) REMOTE_PATH="$2"; shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

if [ -z "$REPO" ]; then
    if git -C "$PROJECT_DIR" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
        REMOTE_URL="$(git -C "$PROJECT_DIR" remote get-url origin 2>/dev/null || true)"
        REPO="$(echo "$REMOTE_URL" | sed 's/.*github.com[:\/]//; s/\.git$//')"
    fi
fi
if [ -z "$REPO" ]; then
    echo "Error: could not detect repo. Provide --repo <user/repo>"
    exit 1
fi

if [ -z "$SSH_USER" ]; then
    SSH_USER="$(whoami)"
fi

if [ -z "$REMOTE_PATH" ]; then
    REMOTE_PATH="$PROJECT_DIR"
fi

LOCAL_VERSION="$(grep '^version ' "$PROJECT_DIR/Cargo.toml" | sed 's/.*"\(.*\)"/\1/')"

echo "=== P2P Mesh Orchestrator ==="
echo "  Repo:           $REPO"
echo "  Local version:  $LOCAL_VERSION"
echo "  Local port:     $PORT"
echo "  SSH user:       $SSH_USER"
echo "  Remote path:    $REMOTE_PATH"
echo ""

P2P_URL="http://127.0.0.1:$PORT"

echo "Fetching known peers from local node..."
STATUS="$(curl -sf "$P2P_URL/api/status")" || {
    echo "Error: cannot reach P2P node at $P2P_URL. Is it running?"
    exit 1
}

echo "$STATUS" | python3 -c "
import sys, json
data = json.load(sys.stdin)
seen = set()
for key in ('known_peers', 'connected_peers'):
    for p in data.get(key, []):
        addr_port = (p['address'], p['port'])
        if addr_port not in seen:
            seen.add(addr_port)
            hostname = p.get('hostname') or p['address']
            print(f'{hostname}|{p[\"address\"]}|{p[\"port\"]}')
" > /tmp/p2p_mesh_nodes.$$.txt

NODE_COUNT="$(wc -l < /tmp/p2p_mesh_nodes.$$.txt | tr -d ' ')"
echo "Found $NODE_COUNT known peer(s)"
echo ""

LATEST_RELEASE=""
LATEST_RELEASE="$(curl -sf "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['tag_name'].lstrip('v'))" 2>/dev/null || echo "")"
if [ -n "$LATEST_RELEASE" ]; then
    echo "Latest GitHub release: v$LATEST_RELEASE"
else
    echo "Could not determine latest GitHub release"
fi
echo ""

SSH_OPTS="-o StrictHostKeyChecking=no -o ConnectTimeout=5 -o BatchMode=yes"

ssh_cmd() {
    local host="$1"; shift
    ssh $SSH_OPTS "$SSH_USER@$host" "$@" </dev/null 2>/dev/null || return 1
}

process_node() {
    local hostname="$1" address="$2" port="$3"
    local label="${hostname} (${address}:${port})"

    echo "--- $label ---"

    local ssh_target="$address"
    if [[ "$ssh_target" == "127.0.0.1" || "$ssh_target" == "localhost" ]]; then
        echo "  Skipping localhost peer"
        echo ""
        return
    fi

    if $DRY_RUN; then
        echo "  [DRY RUN] Would SSH to $SSH_USER@$ssh_target"
        echo ""
        return
    fi

    # Check if node is running
    local pid
    pid="$(ssh_cmd "$ssh_target" "pgrep -f 'p2p_node|p2p-node' 2>/dev/null || true")" || {
        echo "  SSH unreachable"
        echo ""
        return
    }

    if [ -z "$pid" ]; then
        echo "  Node is STOPPED"

        # Look for binary
        local binary=""
        binary="$(ssh_cmd "$ssh_target" "command -v p2p_node 2>/dev/null || command -v p2p-node 2>/dev/null || (test -f $REMOTE_PATH/target/release/p2p_node && echo $REMOTE_PATH/target/release/p2p_node) || (test -f $REMOTE_PATH/target/debug/p2p_node && echo $REMOTE_PATH/target/debug/p2p_node) || true")" || true

        if [ -n "$binary" ]; then
            echo "  Binary: $binary"
            echo "  Starting..."
            local bdir
            bdir="$(dirname "$binary")"
            (ssh_cmd "$ssh_target" "cd '$bdir' && nohup './$(basename "$binary")' > /tmp/p2p_node.log 2>&1 &" 2>/dev/null) && echo "  Started" || echo "  Start FAILED"
        elif ssh_cmd "$ssh_target" "test -d '$REMOTE_PATH'" 2>/dev/null; then
            echo "  Building from $REMOTE_PATH..."
            (ssh_cmd "$ssh_target" "cd '$REMOTE_PATH' && cargo build --release >> /tmp/p2p_node_build.log 2>&1" 2>/dev/null) && {
                echo "  Build OK, starting..."
                (ssh_cmd "$ssh_target" "cd '$REMOTE_PATH' && nohup ./target/release/p2p_node > /tmp/p2p_node.log 2>&1 &" 2>/dev/null) && echo "  Started" || echo "  Start FAILED"
            } || echo "  Build FAILED"
        else
            echo "  No binary or project dir found at $REMOTE_PATH"
        fi
    else
        local pids
        pids="$(echo "$pid" | tr '\n' ' ')"
        echo "  Node is RUNNING (PID(s): $pids)"

        # Try to check version via node's HTTP API
        local version_info
        version_info="$(curl -sf "http://${address}:${port}/api/status" 2>/dev/null || ssh_cmd "$ssh_target" "curl -sf http://127.0.0.1:${port}/api/status 2>/dev/null" 2>/dev/null || true)"

        if [ -n "$version_info" ]; then
            local node_version
            node_version="$(echo "$version_info" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['node']['started_at'])" 2>/dev/null || echo "unknown")"
            if [ -n "$LATEST_RELEASE" ] && [ "$LATEST_RELEASE" != "$LOCAL_VERSION" ]; then
                echo "  Latest: v$LATEST_RELEASE, local: v$LOCAL_VERSION"
                echo "  Triggering update..."
                local update_result
                update_result="$(ssh_cmd "$ssh_target" "curl -sf -X POST http://127.0.0.1:${port}/api/update 2>/dev/null || curl -sf http://127.0.0.1:${port}/api/update 2>/dev/null" 2>/dev/null || true)"
                if echo "$update_result" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('success',False))" 2>/dev/null | grep -q True; then
                    echo "  Update triggered"
                else
                    echo "  Update skipped (no newer release)"
                fi
            else
                echo "  Up to date (v$LOCAL_VERSION)"
            fi
        else
            echo "  HTTP API unreachable at ${address}:${port}"
        fi
    fi
    echo ""
}

while IFS='|' read -r host address port; do
    process_node "$host" "$address" "$port"
done < /tmp/p2p_mesh_nodes.$$.txt

rm -f /tmp/p2p_mesh_nodes.$$.txt

echo "=== Done ==="
