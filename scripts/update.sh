#!/usr/bin/env bash
# Update script for P2P node (Linux/macOS)
# Downloads the latest release binary from GitHub and restarts the node.
#
# Usage: ./update.sh [github_user/repo] [port]
#   Default repo: auto-detected from git remote
#   Default port: 3000

set -euo pipefail

REPO="${1:-}"
PORT="${2:-3000}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_NAME="p2p-node"

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
    Linux:x86_64)     LABEL="linux-x86_64" ;;
    Linux:aarch64)    LABEL="linux-arm64" ;;
    Linux:arm64)      LABEL="linux-arm64" ;;
    Darwin:x86_64)    LABEL="macos-x86_64" ;;
    Darwin:arm64)     LABEL="macos-arm64" ;;
    Darwin:aarch64)   LABEL="macos-arm64" ;;
    *)
        echo "Unsupported platform: $OS $ARCH"
        exit 1
        ;;
esac

echo "=== P2P Node Update ==="
echo "  Repo:    $REPO"
echo "  Label:   $LABEL"
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

DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$APP_NAME-$LABEL"

DOWNLOAD_DIR="$(mktemp -d)"
trap "rm -rf '$DOWNLOAD_DIR'" EXIT

echo "Downloading $APP_NAME-$LABEL..."
curl -sfL "$DOWNLOAD_URL" -o "$DOWNLOAD_DIR/$APP_NAME" || {
    echo "Download failed. Check if $DOWNLOAD_URL exists."
    exit 1
}
chmod +x "$DOWNLOAD_DIR/$APP_NAME"

# Stop the running node
NODE_PID="$(pgrep -f "$APP_NAME" 2>/dev/null || true)"
if [ -n "$NODE_PID" ]; then
    echo "Stopping running node (PID: $NODE_PID)..."
    kill "$NODE_PID" 2>/dev/null || true
    sleep 2
    if kill -0 "$NODE_PID" 2>/dev/null; then
        kill -9 "$NODE_PID" 2>/dev/null || true
        sleep 1
    fi
fi

# Replace binary
OLD_BINARY="$(which "$APP_NAME" 2>/dev/null || echo "$PROJECT_DIR/target/release/$APP_NAME")"
INSTALL_DIR="$(dirname "$OLD_BINARY")"
mkdir -p "$INSTALL_DIR"
echo "Installing to $OLD_BINARY..."
cp "$DOWNLOAD_DIR/$APP_NAME" "$OLD_BINARY"
chmod +x "$OLD_BINARY"

# Restart
echo ""
echo "Starting $APP_NAME on port $PORT..."
cd "$PROJECT_DIR"
nohup "$OLD_BINARY" > /tmp/p2p_node_update.log 2>&1 &
NEW_PID=$!
disown

sleep 2
if kill -0 "$NEW_PID" 2>/dev/null; then
    echo "Node restarted (PID: $NEW_PID)"
    echo "  http://127.0.0.1:$PORT"
    echo "  Logs: /tmp/p2p_node_update.log"
else
    echo "Node failed to start. Check logs: /tmp/p2p_node_update.log"
    exit 1
fi
