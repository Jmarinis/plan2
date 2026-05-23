#!/usr/bin/env bash
# Update script for P2P node (Linux/macOS)
# Downloads the latest release binary from GitHub and restarts the node.
#
# Usage: ./update.sh [github_user/repo] [port]
#   Default repo: the current project's origin remote
#   Default port: 3000

set -euo pipefail

REPO="${1:-}"
PORT="${2:-3000}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_NAME="p2p-node"
NODE_PID=""

# Detect repo from git remote if not provided
if [ -z "$REPO" ]; then
    if git -C "$PROJECT_DIR" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
        REMOTE_URL="$(git -C "$PROJECT_DIR" remote get-url origin 2>/dev/null || true)"
        REPO="$(echo "$REMOTE_URL" | sed 's/.*github.com[:\/]//; s/\.git$//')"
    fi
fi

if [ -z "$REPO" ]; then
    echo "Usage: $0 <github_user/repo> [port]"
    echo "  or run from within a clone of the repo to auto-detect."
    exit 1
fi

# Detect platform
ARCH="$(uname -m)"
OS="$(uname -s)"

case "$OS:$ARCH" in
    Linux:x86_64)     TARGET="x86_64-unknown-linux-gnu" ;;
    Linux:aarch64)    TARGET="aarch64-unknown-linux-gnu" ;;
    Linux:arm64)      TARGET="aarch64-unknown-linux-gnu" ;;
    Darwin:x86_64)    TARGET="x86_64-apple-darwin" ;;
    Darwin:arm64)     TARGET="aarch64-apple-darwin" ;;
    Darwin:aarch64)   TARGET="aarch64-apple-darwin" ;;
    *)
        echo "Unsupported platform: $OS $ARCH"
        exit 1
        ;;
esac

echo "=== P2P Node Update ==="
echo "  Repo:    $REPO"
echo "  Target:  $TARGET"
echo "  Port:    $PORT"
echo ""

# Fetch latest release info
echo "Fetching latest release..."
LATEST="$(curl -sf "https://api.github.com/repos/$REPO/releases/latest")"
TAG="$(echo "$LATEST" | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])" 2>/dev/null)" || {
    echo "Failed to fetch latest release info from $REPO"
    exit 1
}

echo "  Latest:  $TAG"
echo ""

# Find the download URL for our target
DOWNLOAD_URL="$(echo "$LATEST" | python3 -c "
import sys, json
assets = json.load(sys.stdin)['assets']
for a in assets:
    if '$TARGET' in a['name']:
        print(a['browser_download_url'])
        break
" 2>/dev/null)" || {
    echo "No binary found for $TARGET in release $TAG"
    exit 1
}

DOWNLOAD_DIR="$(mktemp -d)"
trap "rm -rf '$DOWNLOAD_DIR'" EXIT

echo "Downloading $APP_NAME-$TARGET..."
curl -sfL "$DOWNLOAD_URL" -o "$DOWNLOAD_DIR/$APP_NAME"
chmod +x "$DOWNLOAD_DIR/$APP_NAME"

# Stop the running node
NODE_PID="$(pgrep -f "$APP_NAME" 2>/dev/null || true)"
if [ -n "$NODE_PID" ]; then
    echo "Stopping running node (PID: $NODE_PID)..."
    kill "$NODE_PID" 2>/dev/null || true
    sleep 2
    # Force kill if still running
    if kill -0 "$NODE_PID" 2>/dev/null; then
        kill -9 "$NODE_PID" 2>/dev/null || true
        sleep 1
    fi
fi

# Replace binary
INSTALL_DIR="$(dirname "$(realpath "$0" 2>/dev/null || readlink -f "$0" 2>/dev/null || echo "$SCRIPT_DIR")")"
if [ -f "$PROJECT_DIR/target/release/$APP_NAME" ]; then
    echo "Replacing project binary..."
    cp "$DOWNLOAD_DIR/$APP_NAME" "$PROJECT_DIR/target/release/$APP_NAME"
fi

# Also copy to a global location if we can find the running binary's path
OLD_BINARY="$(which "$APP_NAME" 2>/dev/null || echo "$PROJECT_DIR/target/release/$APP_NAME")"
if [ -f "$OLD_BINARY" ] && [ ! -w "$OLD_BINARY" ]; then
    echo "Cannot write to $OLD_BINARY, installing to project binary instead."
    OLD_BINARY="$PROJECT_DIR/target/release/$APP_NAME"
    mkdir -p "$(dirname "$OLD_BINARY")"
fi
echo "Installing to $OLD_BINARY..."
cp "$DOWNLOAD_DIR/$APP_NAME" "$OLD_BINARY"
chmod +x "$OLD_BINARY"

# Read env vars
P2P_ADDRESS="${P2P_ADDRESS:-0.0.0.0}"
P2P_HOSTNAME="${P2P_HOSTNAME:-}"

# Restart
echo ""
echo "Starting $APP_NAME on port $PORT..."
cd "$PROJECT_DIR"
nohup "$OLD_BINARY" > /tmp/p2p_node_update.log 2>&1 &
NEW_PID=$!
disown

sleep 2
if kill -0 "$NEW_PID" 2>/dev/null; then
    echo "✅ Node restarted (PID: $NEW_PID)"
    echo "   http://127.0.0.1:$PORT"
    echo "   Logs: /tmp/p2p_node_update.log"
else
    echo "❌ Node failed to start. Check logs: /tmp/p2p_node_update.log"
    exit 1
fi
