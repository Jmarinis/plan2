#!/bin/bash
set -e

BASE_PORT=4000
NODE_COUNT=3

cleanup() {
    echo "Stopping all nodes..."
    for pid in $(pgrep -f "target/debug/p2p_node" | grep -v $$ || true); do
        kill "$pid" 2>/dev/null || true
    done
    sleep 1
    echo "Done."
}

trap cleanup EXIT

cleanup

# Start mesh nodes
for i in $(seq 1 $NODE_COUNT); do
    PORT=$((BASE_PORT + i))
    P2P_PORT=$PORT P2P_HOSTNAME="node-$i" cargo run > /tmp/p2p_node_$i.log 2>&1 &
    echo "Started node $i on port $PORT (PID $!)"
    sleep 1
done

echo ""
echo "=== P2P Mesh Test ==="
echo "Node 1: http://127.0.0.1:$((BASE_PORT + 1))"
echo "Node 2: http://127.0.0.1:$((BASE_PORT + 2))"
echo "Node 3: http://127.0.0.1:$((BASE_PORT + 3))"
echo ""

# Connect node 1 -> node 2 and node 2 -> node 3
echo "Connecting peer mesh..."
curl -s -X POST "http://127.0.0.1:$((BASE_PORT + 1))/api/peers" \
    -H "Content-Type: application/json" \
    -d "{\"address\":\"127.0.0.1\",\"port\":$((BASE_PORT + 2))}" > /dev/null
echo "  Node 1 -> Node 2 connected"

curl -s -X POST "http://127.0.0.1:$((BASE_PORT + 2))/api/peers" \
    -H "Content-Type: application/json" \
    -d "{\"address\":\"127.0.0.1\",\"port\":$((BASE_PORT + 3))}" > /dev/null
echo "  Node 2 -> Node 3 connected"

sleep 2

# Show mesh status
echo ""
echo "=== Mesh Status ==="
for i in $(seq 1 $NODE_COUNT); do
    PORT=$((BASE_PORT + i))
    echo "--- Node $i (port $PORT) ---"
    curl -s "http://127.0.0.1:$PORT/api/status" | python3 -c "
import sys, json
d = json.load(sys.stdin)
nid = d['node']['id'][:8]
conn = [p['id'][:8] for p in d['connected_peers']]
known = [p['id'][:8] for p in d['known_peers']]
print(f\"  Node ID: {nid}...\")
print(f\"  Connected: {conn}\")
print(f\"  Known: {known}\")
"
done

echo ""
echo "=== Monitoring logs (Ctrl+C to stop) ==="
echo ""

# Tail all logs
tail -f /tmp/p2p_node_*.log
