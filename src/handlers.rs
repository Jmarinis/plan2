use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use chrono::Utc;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tracing::{info, warn};
use uuid::Uuid;

use crate::models::{
    self, AddPeerRequest, AddPeerResponse, AppState, DisconnectRequest, DisconnectResponse,
    HandshakeRequest, HandshakeResponse, MeshMcpPeerResult, MeshMcpQuery, MeshMcpResult,
    MeshMcpResponse, Peer, PeerId, PeerNotification, PeerNotificationResponse, RefreshRequest,
    RefreshResponse, RemovePeerRequest, RemovePeerResponse, Session, StatusResponse,
};

pub async fn index_handler() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>P2P Node Status</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #1a1a2e; color: #eee; padding: 20px; }
        .container { max-width: 1200px; margin: 0 auto; }
        h1 { color: #00d9ff; margin-bottom: 20px; }
        h2 { color: #00d9ff; margin: 20px 0 10px; font-size: 1.2em; }
        .card { background: #16213e; border-radius: 8px; padding: 20px; margin-bottom: 20px; }
        th.sortable { cursor: pointer; user-select: none; }
        th.sortable:hover { color: #fff; }
        th .sort-arrow { margin-left: 4px; }
        th .sort-order { font-size: 0.7em; vertical-align: super; margin-left: 2px; color: #888; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 15px; }
        .stat { background: #0f3460; padding: 15px; border-radius: 6px; }
        .stat-label { color: #888; font-size: 0.85em; margin-bottom: 5px; }
        .stat-value { color: #00d9ff; font-size: 1.4em; font-weight: bold; }
        table { width: 100%; border-collapse: collapse; margin-top: 10px; }
        th, td { padding: 12px; text-align: left; border-bottom: 1px solid #0f3460; }
        th { color: #00d9ff; font-weight: 600; }
        .status-connected { color: #00ff88; }
        .status-disconnected { color: #ff6b6b; }
        .form-group { display: flex; gap: 10px; margin-bottom: 15px; }
        input { flex: 1; padding: 10px; border: 1px solid #0f3460; border-radius: 4px; background: #0f3460; color: #eee; }
        button { padding: 10px 20px; background: #00d9ff; color: #1a1a2e; border: none; border-radius: 4px; cursor: pointer; font-weight: bold; }
        button:hover { background: #00b8d4; }
        .refresh { position: fixed; top: 20px; right: 20px; }
    </style>
</head>
<body>
    <div class="container">
        <button class="refresh" onclick="refreshAll()">↻ Refresh</button>
        <h1>🌐 P2P Node Status</h1>
        
        <div class="card">
            <h2>📊 Node Information</h2>
            <div class="grid" id="node-stats">
                <div class="stat"><div class="stat-label">Node ID</div><div class="stat-value" id="node-id">-</div></div>
                <div class="stat"><div class="stat-label">Hostname</div><div class="stat-value" id="node-hostname">-</div></div>
                <div class="stat"><div class="stat-label">Port</div><div class="stat-value" id="node-port">-</div></div>
                <div class="stat"><div class="stat-label">Uptime</div><div class="stat-value" id="node-uptime">-</div></div>
            </div>
            <div style="margin-top: 15px;">
                <div class="stat-label">Service Addresses</div>
                <div id="service-addresses" style="color: #00d9ff; margin-top: 5px; font-family: monospace;"></div>
            </div>
        </div>

        <div class="card">
            <h2>🔗 Add Peer</h2>
            <form class="form-group" onsubmit="addPeer(event)">
                <input type="text" id="peer-address" placeholder="Peer address (e.g., 127.0.0.1)" required>
                <input type="number" id="peer-port" placeholder="Port" value="3000" required style="max-width:100px">
                <button type="submit">Connect</button>
            </form>
        </div>

        <div class="card">
            <h2>✅ Connected Peers (<span id="connected-count">0</span>)</h2>
            <table id="connected-peers">
                <thead><tr><th data-key="id" class="sortable">Peer ID</th><th data-key="hostname" class="sortable">Hostname</th><th data-key="address" class="sortable">Address</th><th data-key="port" class="sortable">Port</th><th data-key="session" class="sortable">Session</th><th data-key="lastSeen" class="sortable">Last Seen</th><th>Actions</th></tr></thead>
                <tbody></tbody>
            </table>
        </div>

        <div class="card">
            <h2>📋 Known Peers (<span id="known-count">0</span>)</h2>
            <table id="known-peers">
                <thead><tr><th data-key="id" class="sortable">Peer ID</th><th data-key="hostname" class="sortable">Hostname</th><th data-key="address" class="sortable">Address</th><th data-key="port" class="sortable">Port</th><th data-key="status" class="sortable">Status</th><th data-key="lastSeen" class="sortable">Last Seen</th><th>Actions</th></tr></thead>
                <tbody></tbody>
            </table>
        </div>
    </div>
    <script>
        const connectedSortState = [];
        const knownSortState = [];

        function getSortValue(p, key) {
            switch (key) {
                case 'id': return p.id;
                case 'hostname': return (p.hostname || '').toLowerCase();
                case 'address': return p.address;
                case 'port': return p.port;
                case 'session': return p.session_id || '';
                case 'lastSeen': return new Date(p.last_seen).getTime();
                case 'status': return p.connected ? 0 : 1;
                default: return '';
            }
        }

        function comparePeers(a, b, sortState) {
            for (const {key, dir} of sortState) {
                const va = getSortValue(a, key);
                const vb = getSortValue(b, key);
                if (va < vb) return -1 * dir;
                if (va > vb) return 1 * dir;
            }
            return 0;
        }

        function handleSortClick(tableId, sortState, key, event) {
            const idx = sortState.findIndex(s => s.key === key);
            if (event.ctrlKey || event.metaKey) {
                if (idx >= 0) {
                    const existing = sortState[idx];
                    sortState.splice(idx, 1);
                    sortState.push({key, dir: -existing.dir});
                } else {
                    sortState.push({key, dir: -1});
                }
            } else {
                if (idx === 0) {
                    sortState[0].dir *= -1;
                } else {
                    sortState.length = 0;
                    sortState.push({key, dir: -1});
                }
            }
            renderTables();
        }

        function renderArrow(key, sortState) {
            const idx = sortState.findIndex(s => s.key === key);
            if (idx === -1) return '';
            const dir = sortState[idx].dir;
            const arrow = dir === -1 ? '&#9650;' : '&#9660;';
            const order = sortState.length > 1 ? `<span class="sort-order">${idx + 1}</span>` : '';
            return `<span class="sort-arrow">${arrow}${order}</span>`;
        }

        function initSortHeaders(tableId, sortState) {
            const headers = document.querySelectorAll(`#${tableId} th.sortable`);
            headers.forEach(th => {
                th.onclick = (e) => handleSortClick(tableId, sortState, th.dataset.key, e);
            });
        }

        let cachedData = null;

        async function loadStatus() {
            const res = await fetch('/api/status');
            const data = await res.json();
            cachedData = data;
            renderTables();
        }

        function renderTables() {
            if (!cachedData) return;
            const data = cachedData;

            document.getElementById('node-id').textContent = data.node.id.slice(0, 8) + '...';
            document.getElementById('node-hostname').textContent = data.node.hostname;
            document.title = `P2P Node - ${data.node.hostname}`;
            document.getElementById('node-port').textContent = data.node.port;
            document.getElementById('node-uptime').textContent = data.node.uptime_seconds + 's';
            
            const addresses = data.node.service_addresses || [];
            document.getElementById('service-addresses').innerHTML = addresses
                .map(addr => `<span style="display: inline-block; background: #0f3460; padding: 5px 10px; border-radius: 4px; margin: 3px;">${addr}:${data.node.port}</span>`)
                .join('') || '<span style="color: #888;">No addresses available</span>';

            const connectedPeers = [...data.connected_peers].sort((a, b) => comparePeers(a, b, connectedSortState));
            const connectedBody = document.querySelector('#connected-peers tbody');
            connectedBody.innerHTML = connectedPeers.map(p =>
                `<tr><td>${p.id.slice(0,16)}...</td><td>${p.hostname || '-'}</td><td>${p.address}</td><td>${p.port}</td>
                <td class="status-connected">${p.session_id ? p.session_id.slice(0,8)+'...' : '-'}</td>
                <td>${new Date(p.last_seen).toLocaleTimeString()}</td>
                <td><button onclick="disconnectPeer('${p.id}','${p.address}',${p.port})" style="background:#ff6b6b;color:white;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;">Disconnect</button></td></tr>`
            ).join('') || '<tr><td colspan="7">No connected peers</td></tr>';
            document.getElementById('connected-count').textContent = data.connected_peers.length;

            const knownPeersOnly = data.known_peers.filter(p => !p.connected);
            const sortedKnown = [...knownPeersOnly].sort((a, b) => comparePeers(a, b, knownSortState));
            const knownBody = document.querySelector('#known-peers tbody');
            knownBody.innerHTML = sortedKnown.map(p =>
                `<tr><td>${p.id.slice(0,16)}...</td><td>${p.hostname || '-'}</td><td>${p.address}</td><td>${p.port}</td>
                <td class="status-disconnected">Disconnected</td>
                <td>${new Date(p.last_seen).toLocaleTimeString()}</td>
                <td>
                    <button onclick="connectPeer('${p.id}','${p.address}',${p.port})" style="background:#00d9ff;color:#1a1a2e;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;font-weight:bold;">Connect</button>
                    <button onclick="removePeer('${p.id}','${p.address}',${p.port})" style="background:#ff6b6b;color:white;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;margin-left:5px;">Remove</button>
                </td></tr>`
            ).join('') || '<tr><td colspan="7">No known peers</td></tr>';
            document.getElementById('known-count').textContent = knownPeersOnly.length;

            document.querySelectorAll('#connected-peers th.sortable, #known-peers th.sortable').forEach(th => {
                const tableId = th.closest('table').id;
                const ss = tableId === 'connected-peers' ? connectedSortState : knownSortState;
                th.innerHTML = th.innerHTML.replace(/<span class="sort-arrow">.*<\/span>/, '') + renderArrow(th.dataset.key, ss);
            });
        }

        async function disconnectPeer(peerId, address, port) {
            console.log('Disconnecting peer:', peerId);
            try {
                const response = await fetch('/api/peers/disconnect', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({peer_id: peerId, address, port})
                });
                loadStatus();
            } catch (error) {
                console.error('Disconnect failed:', error);
            }
        }

        async function removePeer(peerId, address, port) {
            try {
                const response = await fetch('/api/peers/remove', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({peer_id: peerId, address, port})
                });
                loadStatus();
            } catch (error) {
                console.error('Remove failed:', error);
            }
        }

        async function connectPeer(peerId, address, port) {
            try {
                const response = await fetch('/api/peers/connect', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({peer_id: peerId, address, port})
                });
                loadStatus();
            } catch (error) {
                console.error('Connect failed:', error);
            }
        }

        async function addPeer(e) {
            e.preventDefault();
            const address = document.getElementById('peer-address').value;
            const port = document.getElementById('peer-port').value;
            await fetch('/api/peers', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({address, port: parseInt(port)})
            });
            document.getElementById('peer-address').value = '';
            document.getElementById('peer-port').value = '';
            loadStatus();
        }

        initSortHeaders('connected-peers', connectedSortState);
        initSortHeaders('known-peers', knownSortState);

        async function refreshAll() {
            try {
                await fetch('/api/refresh', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({request_id: crypto.randomUUID()})
                });
            } catch (e) {
                console.error('Refresh failed:', e);
            }
            setTimeout(loadStatus, 1000);
        }

        loadStatus();
        setInterval(loadStatus, 5000);
    </script>
</body>
</html>"#,
    )
}

/// Process known peers received during a handshake.
/// Attempts to connect to any peers not already in our peer list.
pub async fn connect_to_unknown_peers(
    state: &AppState,
    known_peers: Vec<models::PeerInfo>,
    exclude_addr: &str,
    exclude_port: u16,
) {
    for kp in known_peers {
        if kp.address == exclude_addr && kp.port == exclude_port {
            continue;
        }
        if kp.address.parse::<std::net::IpAddr>().is_err() {
            warn!("Skipping discovered peer with non-IP address: {}", kp.address);
            continue;
        }

        let kp_id = kp
            .node_id
            .clone()
            .unwrap_or_else(|| models::generate_peer_id(&kp.address, kp.port));
        let our_node_id = state.node_state.read().await.id.clone();

        let already_known = {
            let peers = state.peers.read().await;
            peers.contains_key(&kp_id) || peers.values().any(|p| p.address == kp.address && p.port == kp.port)
        };
        if already_known || kp_id == our_node_id {
            continue;
        }

        info!(
            "Discovered unknown peer {}:{} (id: {}), attempting connection...",
            kp.address, kp.port, &kp_id[..8.min(kp_id.len())]
        );

        let node_state = state.node_state.read().await;
        let our_address = if node_state.address == "0.0.0.0" {
            node_state
                .service_addresses
                .first()
                .cloned()
                .unwrap_or_else(|| "127.0.0.1".to_string())
        } else {
            node_state.address.clone()
        };
        let our_port = node_state.port;
        let our_hostname = node_state.hostname.clone();
        drop(node_state);

        match state
            .http_client
            .post(format!("http://{}:{}/api/handshake", kp.address, kp.port))
            .json(&HandshakeRequest {
                node_id: our_node_id.clone(),
                address: our_address,
                port: our_port,
                hostname: our_hostname,
            })
            .send()
            .await
        {
            Ok(resp) => {
            if let Ok(handshake) = resp.json::<HandshakeResponse>().await {
                    if handshake.accepted {
                        let remote_id = handshake
                            .node_id
                            .unwrap_or_else(|| models::generate_peer_id(&kp.address, kp.port));

                        let new_peer_info = models::PeerInfo {
                            address: kp.address.clone(),
                            port: kp.port,
                            hostname: handshake.hostname.clone(),
                            node_id: Some(remote_id.clone()),
                        };

                        let mut peers = state.peers.write().await;
                        if let Some(existing) = peers.get_mut(&remote_id) {
                            existing.address = kp.address.clone();
                            existing.port = kp.port;
                            existing.connected = true;
                            existing.hostname = new_peer_info.hostname.clone();
                            existing.session_id = handshake.session_id.clone();
                            existing.last_seen = Utc::now();
                            existing.health_check_failures = 0;
                        } else {
                            let mut peer = Peer::new(kp.address.clone(), kp.port);
                            peer.id = remote_id.clone();
                            peer.connected = true;
                            peer.hostname = new_peer_info.hostname.clone();
                            peer.session_id = handshake.session_id.clone();

                            if let Some(session_id) = &peer.session_id {
                                let mut sessions = state.sessions.write().await;
                                sessions.insert(session_id.clone(), Session::new(peer.id.clone()));
                            }

                            peers.insert(peer.id.clone(), peer);
                        }

                        broadcast_new_peer(state, &remote_id, &new_peer_info);

                        info!("Connected to discovered peer {}:{}", kp.address, kp.port);
                    }
                }
            }
            Err(e) => {
                let mut peers = state.peers.write().await;
                let stale_ids: Vec<PeerId> = peers
                    .iter()
                    .filter(|(id, p)| p.address == kp.address && p.port == kp.port && *id != &kp_id)
                    .map(|(id, _)| id.clone())
                    .collect();
                for stale_id in stale_ids {
                    if let Some(stale) = peers.remove(&stale_id) {
                        if let Some(sid) = stale.session_id {
                            let mut sessions = state.sessions.write().await;
                            sessions.remove(&sid);
                        }
                    }
                }
                peers.entry(kp_id).or_insert_with(|| {
                    let mut p = Peer::new(kp.address.clone(), kp.port);
                    p.hostname = kp.hostname;
                    p.last_seen = Utc::now();
                    p
                });
                warn!(
                    "Failed to connect to discovered peer {}:{} - {}",
                    kp.address, kp.port, e
                );
            }
        }
    }
}

/// Broadcast a newly connected peer to all other connected peers.
/// Spawns a background task so the caller is not blocked.
pub fn broadcast_new_peer(state: &AppState, new_peer_id: &str, new_peer: &models::PeerInfo) {
    let state = state.clone();
    let new_peer_id = new_peer_id.to_string();
    let new_peer = new_peer.clone();

    tokio::spawn(async move {
        let node_id = state.node_state.read().await.id.clone();
        let connected: Vec<(String, String, u16)> = {
            let peers = state.peers.read().await;
            peers
                .values()
                .filter(|p| p.connected && p.id != new_peer_id)
                .map(|p| (p.id.clone(), p.address.clone(), p.port))
                .collect()
        };

        if connected.is_empty() {
            return;
        }

        let notification = PeerNotification {
            peer: new_peer,
            sender_id: node_id,
        };

        for (_peer_id, addr, port) in &connected {
            let url = format!("http://{}:{}/api/peers/notify", addr, port);
            if let Err(e) = state
                .http_client
                .post(&url)
                .json(&notification)
                .send()
                .await
            {
                warn!("Failed to notify peer {}:{} about new peer: {}", addr, port, e);
            }
        }
    });
}

pub async fn notify_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<PeerNotification>,
) -> Json<PeerNotificationResponse> {
    let peer_info = &payload.peer;
    let kp_id = peer_info
        .node_id
        .clone()
        .unwrap_or_else(|| models::generate_peer_id(&peer_info.address, peer_info.port));
    let our_id = state.node_state.read().await.id.clone();

    if kp_id == our_id {
        return Json(PeerNotificationResponse { accepted: false });
    }

    let mut peers = state.peers.write().await;

    if let Some(existing) = peers.get_mut(&kp_id) {
        if existing.address == peer_info.address && existing.port == peer_info.port {
            return Json(PeerNotificationResponse { accepted: true });
        }
    }

    let stale_ids: Vec<PeerId> = peers
        .iter()
        .filter(|(id, p)| p.address == peer_info.address && p.port == peer_info.port && *id != &kp_id)
        .map(|(id, _)| id.clone())
        .collect();
    for stale_id in stale_ids {
        if let Some(stale) = peers.remove(&stale_id) {
            if let Some(sid) = stale.session_id {
                let mut sessions = state.sessions.write().await;
                sessions.remove(&sid);
            }
        }
    }

    peers.entry(kp_id).or_insert_with(|| {
        let mut p = Peer::new(peer_info.address.clone(), peer_info.port);
        p.hostname = peer_info.hostname.clone();
        p.last_seen = Utc::now();
        info!(
            "Discovered peer via notification: {}:{}",
            peer_info.address, peer_info.port
        );
        p
    });

    Json(PeerNotificationResponse { accepted: true })
}

pub async fn refresh_handler(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Json<RefreshResponse> {
    let our_id = state.node_state.read().await.id.clone();

    {
        let mut seen = state.seen_refresh_ids.write().await;
        let now = Instant::now();
        seen.retain(|_, ts| now.duration_since(*ts) < Duration::from_secs(60));
        if seen.contains_key(&payload.request_id) {
            return Json(RefreshResponse {
                accepted: false,
                message: "Refresh already in progress".to_string(),
            });
        }
        seen.insert(payload.request_id.clone(), now);
    }

    info!("Refresh requested (id: {}), re-handshaking with connected peers...", &payload.request_id[..8]);

    let connected: Vec<(String, String, u16)> = {
        let peers = state.peers.read().await;
        peers
            .values()
            .filter(|p| p.connected)
            .map(|p| (p.id.clone(), p.address.clone(), p.port))
            .collect()
    };

    for (peer_id, addr, port) in &connected {
        let node_state = state.node_state.read().await;
        let our_address = if node_state.address == "0.0.0.0" {
            node_state
                .service_addresses
                .first()
                .cloned()
                .unwrap_or_else(|| "127.0.0.1".to_string())
        } else {
            node_state.address.clone()
        };
        let our_port = node_state.port;
        let our_hostname = node_state.hostname.clone();
        drop(node_state);

        match state
            .http_client
            .post(format!("http://{}:{}/api/handshake", addr, port))
            .json(&HandshakeRequest {
                node_id: our_id.clone(),
                address: our_address,
                port: our_port,
                hostname: our_hostname,
            })
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(handshake) = resp.json::<HandshakeResponse>().await {
                    if handshake.accepted {
                        let mut peers = state.peers.write().await;
                        if let Some(p) = peers.get_mut(peer_id) {
                            p.session_id = handshake.session_id.clone();
                            p.health_check_failures = 0;
                            if let Some(hostname) = handshake.hostname {
                                p.hostname = Some(hostname);
                            }
                        }
                        drop(peers);

                        if let Some(session_id) = handshake.session_id {
                            let mut sessions = state.sessions.write().await;
                            sessions.insert(session_id, Session::new(peer_id.clone()));
                        }

                        if let Some(kp) = handshake.known_peers {
                            let state = state.clone();
                            let exclude_addr = addr.clone();
                            let exclude_port = *port;
                            tokio::spawn(async move {
                                connect_to_unknown_peers(&state, kp, &exclude_addr, exclude_port).await;
                            });
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Refresh: failed to handshake with peer {}:{} - {}", addr, port, e);
            }
        }
    }

    let state_clone = state.clone();
    let request_id = payload.request_id.clone();
    tokio::spawn(async move {
        let connected: Vec<(String, u16)> = {
            let peers = state_clone.peers.read().await;
            peers
                .values()
                .filter(|p| p.connected)
                .map(|p| (p.address.clone(), p.port))
                .collect()
        };
        for (addr, port) in connected {
            let _ = state_clone
                .http_client
                .post(format!("http://{}:{}/api/refresh", addr, port))
                .json(&RefreshRequest {
                    request_id: request_id.clone(),
                })
                .send()
                .await;
        }
    });

    Json(RefreshResponse {
        accepted: true,
        message: format!("Refresh initiated with {} connected peer(s)", connected.len()),
    })
}

async fn execute_mcp_tool_locally(
    state: &AppState,
    tool_name: &str,
    _arguments: &serde_json::Value,
) -> MeshMcpResult {
    let port = state.node_state.read().await.port;
    let base_url = format!("http://127.0.0.1:{}", port);

    match tool_name {
        "get_status" => match state
            .http_client
            .get(format!("{}/api/status", base_url))
            .send()
            .await
        {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(data) => MeshMcpResult {
                    success: true,
                    data,
                    error: None,
                },
                Err(e) => MeshMcpResult {
                    success: false,
                    data: serde_json::Value::Null,
                    error: Some(format!("Failed to parse status: {}", e)),
                },
            },
            Err(e) => MeshMcpResult {
                success: false,
                data: serde_json::Value::Null,
                error: Some(format!("Local status request failed: {}", e)),
            },
        },
        "refresh" => {
            let request_id = Uuid::new_v4().to_string();
            match state
                .http_client
                .post(format!("{}/api/refresh", base_url))
                .json(&RefreshRequest { request_id })
                .send()
                .await
            {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(data) => MeshMcpResult {
                        success: true,
                        data,
                        error: None,
                    },
                    Err(e) => MeshMcpResult {
                        success: false,
                        data: serde_json::Value::Null,
                        error: Some(format!("Failed to parse refresh response: {}", e)),
                    },
                },
                Err(e) => MeshMcpResult {
                    success: false,
                    data: serde_json::Value::Null,
                    error: Some(format!("Local refresh request failed: {}", e)),
                },
            }
        }
        _ => MeshMcpResult {
            success: false,
            data: serde_json::Value::Null,
            error: Some(format!("Unknown tool: {}", tool_name)),
        },
    }
}

pub async fn mcp_query_handler(
    State(state): State<AppState>,
    Json(query): Json<MeshMcpQuery>,
) -> Json<MeshMcpResponse> {
    let our_id = state.node_state.read().await.id.clone();

    {
        let mut seen = state.seen_mcp_ids.write().await;
        let now = Instant::now();
        seen.retain(|_, ts| now.duration_since(*ts) < Duration::from_secs(60));
        if seen.contains_key(&query.request_id) {
            return Json(MeshMcpResponse {
                request_id: query.request_id,
                node_id: our_id,
                hop_count: query.hop_count,
                local: MeshMcpResult {
                    success: false,
                    data: serde_json::Value::Null,
                    error: Some("Duplicate request".into()),
                },
                peers: vec![],
            });
        }
        seen.insert(query.request_id.clone(), now);
    }

    let local = execute_mcp_tool_locally(&state, &query.tool_name, &query.arguments).await;

    let mut peer_results: Vec<MeshMcpPeerResult> = vec![];
    if query.hop_count > 0 {
        let connected: Vec<(String, String, u16)> = {
            let peers = state.peers.read().await;
            peers
                .values()
                .filter(|p| p.connected)
                .map(|p| (p.id.clone(), p.address.clone(), p.port))
                .collect()
        };

        for (peer_id, addr, port) in &connected {
            let peer_query = MeshMcpQuery {
                request_id: query.request_id.clone(),
                hop_count: query.hop_count - 1,
                tool_name: query.tool_name.clone(),
                arguments: query.arguments.clone(),
            };

            match state
                .http_client
                .post(format!("http://{}:{}/api/mcp/query", addr, port))
                .json(&peer_query)
                .send()
                .await
            {
                Ok(resp) => {
                    match resp.json::<MeshMcpResponse>().await {
                        Ok(peer_resp) => {
                            let data = serde_json::to_value(&peer_resp).unwrap_or_default();
                            peer_results.push(MeshMcpPeerResult {
                                node_id: peer_id.clone(),
                                result: MeshMcpResult {
                                    success: true,
                                    data,
                                    error: None,
                                },
                            });
                        }
                        Err(e) => peer_results.push(MeshMcpPeerResult {
                            node_id: peer_id.clone(),
                            result: MeshMcpResult {
                                success: false,
                                data: serde_json::Value::Null,
                                error: Some(format!("Bad peer response: {}", e)),
                            },
                        }),
                    }
                }
                Err(e) => peer_results.push(MeshMcpPeerResult {
                    node_id: peer_id.clone(),
                    result: MeshMcpResult {
                        success: false,
                        data: serde_json::Value::Null,
                        error: Some(format!("Peer unreachable: {}", e)),
                    },
                }),
            }
        }
    }

    Json(MeshMcpResponse {
        request_id: query.request_id,
        node_id: our_id,
        hop_count: query.hop_count,
        local,
        peers: peer_results,
    })
}

pub async fn status_handler(State(state): State<AppState>) -> Json<StatusResponse> {
    let node_state = state.node_state.read().await.clone();
    let peers = state.peers.read().await;
    let sessions = state.sessions.read().await.clone();

    let connected_peers: Vec<Peer> = peers.values().filter(|p| p.connected).cloned().collect();
    let known_peers: Vec<Peer> = peers.values().filter(|p| !p.connected).cloned().collect();
    let active_sessions: Vec<Session> = sessions.values().cloned().collect();

    Json(StatusResponse {
        node: node_state,
        connected_peers,
        known_peers,
        active_sessions,
    })
}

pub async fn add_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<AddPeerRequest>,
) -> impl IntoResponse {
    let mut peer = Peer::new(payload.address.clone(), payload.port);

    let node_state = state.node_state.read().await;
    let our_address = if node_state.address == "0.0.0.0" {
        node_state
            .service_addresses
            .first()
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string())
    } else {
        node_state.address.clone()
    };
    let our_port = node_state.port;
    let our_hostname = node_state.hostname.clone();
    let our_node_id = node_state.id.clone();
    drop(node_state);

    match state
        .http_client
        .post(format!("http://{}:{}/api/handshake", peer.address, peer.port))
        .json(&HandshakeRequest {
            node_id: our_node_id.clone(),
            address: our_address,
            port: our_port,
            hostname: our_hostname,
        })
        .send()
        .await
    {
        Ok(resp) => {
        if let Ok(handshake) = resp.json::<HandshakeResponse>().await {
                if handshake.accepted {
                    let remote_node_id = handshake
                        .node_id
                        .clone()
                        .unwrap_or_else(|| peer.id.clone());

                    let already_known = {
                        let peers = state.peers.read().await;
                        peers.contains_key(&remote_node_id)
                    };

                    if already_known {
                        let mut peers = state.peers.write().await;
                        if let Some(existing) = peers.get_mut(&remote_node_id) {
                            existing.address = payload.address.clone();
                            existing.port = payload.port;
                            existing.connected = true;
                            existing.hostname = handshake.hostname.clone();
                            existing.session_id = handshake.session_id.clone();
                            existing.last_seen = Utc::now();
                            existing.health_check_failures = 0;
                            peer = existing.clone();
                        }
                    } else {
                        peer.connected = true;
                        peer.id = remote_node_id;
                        peer.session_id = handshake.session_id;
                        peer.hostname = handshake.hostname;
                    }

                    if let Some(session_id) = &peer.session_id {
                        let mut sessions = state.sessions.write().await;
                        sessions.insert(session_id.clone(), Session::new(peer.id.clone()));
                    }

                    let new_peer_info = models::PeerInfo {
                        address: payload.address.clone(),
                        port: payload.port,
                        hostname: peer.hostname.clone(),
                        node_id: Some(peer.id.clone()),
                    };
                    broadcast_new_peer(&state, &peer.id, &new_peer_info);

                    let received_known_peers = handshake.known_peers.clone();

                    if let Some(known_peers) = handshake.known_peers {
                        let mut peers = state.peers.write().await;
                        for kp in known_peers {
                            let kp_id = kp.node_id.as_ref().cloned().unwrap_or_else(|| {
                                models::generate_peer_id(&kp.address, kp.port)
                            });
                            if kp_id == our_node_id || kp_id == peer.id {
                                continue;
                            }
                            if kp.address.parse::<std::net::IpAddr>().is_ok()
                                && kp.address != payload.address
                            {
                                let stale_ids: Vec<PeerId> = peers
                                    .iter()
                                    .filter(|(id, p)| p.address == kp.address && p.port == kp.port && *id != &kp_id)
                                    .map(|(id, _)| id.clone())
                                    .collect();
                                for stale_id in stale_ids {
                                    if let Some(stale) = peers.remove(&stale_id) {
                                        if let Some(sid) = stale.session_id {
                                            let mut sessions = state.sessions.write().await;
                                            sessions.remove(&sid);
                                        }
                                    }
                                }
                                peers.entry(kp_id).or_insert_with(|| {
                                    let mut p = Peer::new(kp.address.clone(), kp.port);
                                    p.hostname = kp.hostname;
                                    p.last_seen = Utc::now();
                                    p
                                });
                            } else {
                                warn!(
                                    "Skipping known peer with non-IP address: {}",
                                    kp.address
                                );
                            }
                        }
                    }

                    info!("Connected to peer {}:{}", peer.address, peer.port);

                    if let Some(kp) = received_known_peers {
                        let state = state.clone();
                        let exclude_addr = payload.address.clone();
                        tokio::spawn(async move {
                            connect_to_unknown_peers(&state, kp, &exclude_addr, payload.port).await;
                        });
                    }
                }
            }
        }
        Err(e) => {
            warn!(
                "Failed to connect to peer {}:{} - {}",
                peer.address, peer.port, e
            );
        }
    }

    let mut peers = state.peers.write().await;
    let peer_id = peer.id.clone();
    peers.entry(peer_id.clone()).or_insert(peer.clone());

    (
        StatusCode::OK,
        Json(AddPeerResponse {
            success: true,
            peer: Some(peer),
            message: format!("Peer {} added", peer_id),
        }),
    )
}

pub async fn remove_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<RemovePeerRequest>,
) -> impl IntoResponse {
    let mut peers = state.peers.write().await;

    if let Some(peer) = peers.remove(&payload.peer_id) {
        if peer.connected {
            if let Some(session_id) = &peer.session_id {
                let _ = state
                    .http_client
                    .post(format!(
                        "http://{}:{}/api/disconnect-session",
                        peer.address, peer.port
                    ))
                    .json(&DisconnectRequest {
                        node_id: state.node_state.read().await.id.clone(),
                        session_id: session_id.clone(),
                        reason: "Peer being removed".to_string(),
                    })
                    .send()
                    .await;
            }
        }

        if let Some(session_id) = &peer.session_id {
            let mut sessions = state.sessions.write().await;
            sessions.remove(session_id);
        }

        info!("Removed peer {}:{}", peer.address, peer.port);
    } else if let (Some(addr), Some(port)) = (&payload.address, payload.port) {
        let found: Vec<String> = peers
            .iter()
            .filter(|(_, p)| p.address == *addr && p.port == port)
            .map(|(id, _)| id.clone())
            .collect();
        for id in found {
            if let Some(peer) = peers.remove(&id) {
                if let Some(session_id) = peer.session_id {
                    let mut sessions = state.sessions.write().await;
                    sessions.remove(&session_id);
                }
                info!("Removed peer {}:{} (found by address:port)", addr, port);
            }
        }
    } else {
        info!("Peer {} not found for removal (already removed) — address={:?} port={:?}", payload.peer_id, payload.address, payload.port);
    }

    (
        StatusCode::OK,
        Json(RemovePeerResponse {
            success: true,
            message: format!("Peer {} removed", payload.peer_id),
        }),
    )
}

pub async fn disconnect_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<RemovePeerRequest>,
) -> impl IntoResponse {
    let peer_id = payload.peer_id.clone();
    let mut peers = state.peers.write().await;

    let target = if let Some(peer) = peers.get_mut(&peer_id) {
        Some(peer.id.clone())
    } else if let (Some(addr), Some(port)) = (&payload.address, payload.port) {
        peers
            .iter()
            .find(|(_, p)| p.address == *addr && p.port == port)
            .map(|(id, _)| id.clone())
    } else {
        None
    };

    if let Some(found_id) = target {
        if let Some(peer) = peers.get_mut(&found_id) {
            info!(
                "Disconnect request for peer {} (connected: {})",
                found_id, peer.connected
            );

            if let Some(session_id) = &peer.session_id {
                let our_id = state.node_state.read().await.id.clone();
                let _ = state
                    .http_client
                    .post(format!(
                        "http://{}:{}/api/disconnect-session",
                        peer.address, peer.port
                    ))
                    .json(&DisconnectRequest {
                        node_id: our_id,
                        session_id: session_id.clone(),
                        reason: "Peer initiated disconnect".to_string(),
                    })
                    .send()
                    .await;
            }

            if let Some(session_id) = peer.session_id.take() {
                let mut sessions = state.sessions.write().await;
                sessions.remove(&session_id);
            }

            peer.connected = false;

            info!("Disconnected from peer {}:{}", peer.address, peer.port);
        }
    } else {
        info!("Peer {} not found for disconnect (already removed) — address={:?} port={:?}", peer_id, payload.address, payload.port);
    }

    (
        StatusCode::OK,
        Json(RemovePeerResponse {
            success: true,
            message: format!("Disconnected from peer {}", peer_id),
        }),
    )
}

pub async fn connect_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<RemovePeerRequest>,
) -> impl IntoResponse {
    let (peer_address, peer_port, peer_connected, peer_id) = {
        let peers = state.peers.read().await;

        let found = peers.get(&payload.peer_id).or_else(|| {
            payload.address.as_ref().and_then(|addr| {
                payload.port.and_then(|port| {
                    peers.values().find(|p| p.address == *addr && p.port == port)
                })
            })
        });

        if let Some(peer) = found {
            (peer.address.clone(), peer.port, peer.connected, peer.id.clone())
        } else {
            return (
                StatusCode::NOT_FOUND,
                Json(RemovePeerResponse {
                    success: false,
                    message: "Peer not found".to_string(),
                }),
            );
        }
    };

    if peer_connected {
            return (
                StatusCode::OK,
                Json(RemovePeerResponse {
                    success: true,
                    message: format!("Peer {} is already connected", payload.peer_id),
                }),
            );
        }

        let node_state = state.node_state.read().await;
        let our_address = if node_state.address == "0.0.0.0" {
            node_state
                .service_addresses
                .first()
                .cloned()
                .unwrap_or_else(|| "127.0.0.1".to_string())
        } else {
            node_state.address.clone()
        };
        let our_port = node_state.port;
        let our_hostname = node_state.hostname.clone();
        let our_node_id = node_state.id.clone();
        drop(node_state);

        info!("Attempting to connect to peer {}:{}...", peer_address, peer_port);

        match state
            .http_client
            .post(format!("http://{}:{}/api/handshake", peer_address, peer_port))
            .json(&HandshakeRequest {
                node_id: our_node_id,
                address: our_address,
                port: our_port,
                hostname: our_hostname,
            })
            .send()
            .await
        {
            Ok(resp) => {
                match resp.json::<HandshakeResponse>().await {
                    Ok(handshake) => {
                        if handshake.accepted {
                            let known_to_exchange = handshake.known_peers.clone();
                            let remote_node_id = handshake.node_id.clone().unwrap_or_default();

                            let mut peers = state.peers.write().await;
                            let effective_id = if !remote_node_id.is_empty() && peers.contains_key(&remote_node_id) && remote_node_id != peer_id {
                                remote_node_id.clone()
                            } else {
                                peer_id.clone()
                            };

                            if effective_id != peer_id {
                                peers.remove(&peer_id);
                            }

                            let new_peer_info = models::PeerInfo {
                                address: peer_address.clone(),
                                port: peer_port,
                                hostname: handshake.hostname.clone(),
                                node_id: Some(effective_id.clone()),
                            };

                            if let Some(p) = peers.get_mut(&effective_id) {
                                p.connected = true;
                                p.session_id = handshake.session_id.clone();
                                p.hostname = new_peer_info.hostname.clone();
                                p.last_seen = Utc::now();
                                p.health_check_failures = 0;
                            } else {
                                let mut p = Peer::new(peer_address.clone(), peer_port);
                                p.id = effective_id.clone();
                                p.connected = true;
                                p.hostname = new_peer_info.hostname.clone();
                                p.session_id = handshake.session_id.clone();
                                peers.insert(effective_id.clone(), p);
                            }

                            if let Some(session_id) = handshake.session_id {
                                let mut sessions = state.sessions.write().await;
                                sessions.insert(session_id, Session::new(String::new()));
                            }

                            broadcast_new_peer(
                                &state,
                                &effective_id,
                                &new_peer_info,
                            );

                            info!("Connected to peer {}:{}", peer_address, peer_port);

                            if let Some(kp) = known_to_exchange {
                                let state = state.clone();
                                let exclude_addr = peer_address.clone();
                                tokio::spawn(async move {
                                    connect_to_unknown_peers(&state, kp, &exclude_addr, peer_port).await;
                                });
                            }

                            return (
                                StatusCode::OK,
                                Json(RemovePeerResponse {
                                    success: true,
                                    message: format!("Connected to peer {}", payload.peer_id),
                                }),
                            );
                        } else {
                            warn!("Peer {}:{} rejected handshake", peer_address, peer_port);
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to parse handshake response from {}:{} - {}",
                            peer_address, peer_port, e
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to connect to peer {}:{} - {}",
                    peer_address, peer_port, e
                );
            }
        }

        (
            StatusCode::OK,
            Json(RemovePeerResponse {
                success: false,
                message: format!("Failed to connect to peer {}", payload.peer_id),
            }),
        )
}

pub async fn disconnect_session_handler(
    State(state): State<AppState>,
    Json(payload): Json<DisconnectRequest>,
) -> Json<DisconnectResponse> {
    let mut peers = state.peers.write().await;
    let mut disconnected = false;

    for peer in peers.values_mut() {
        if peer.session_id.as_ref() == Some(&payload.session_id)
            || peer.id == payload.node_id
        {
            disconnected = true;
            peer.connected = false;
            peer.session_id = None;
            info!(
                "Peer {}:{} disconnected (session only): {}",
                peer.address, peer.port, payload.reason
            );
            break;
        }
    }

    if disconnected {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&payload.session_id);
    }

    Json(DisconnectResponse { accepted: disconnected })
}

pub async fn disconnect_handler(
    State(state): State<AppState>,
    Json(payload): Json<DisconnectRequest>,
) -> Json<DisconnectResponse> {
    let mut peers = state.peers.write().await;
    let mut removed = false;

    peers.retain(|_, peer| {
        if peer.session_id.as_ref() == Some(&payload.session_id)
            || peer.id == payload.node_id
        {
            removed = true;
            info!(
                "Peer {}:{} disconnected: {}",
                peer.address, peer.port, payload.reason
            );
            false
        } else {
            true
        }
    });

    if removed {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&payload.session_id);
    }

    Json(DisconnectResponse { accepted: removed })
}

pub async fn handshake_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(payload): Json<HandshakeRequest>,
) -> Json<HandshakeResponse> {
    let node_id = state.node_state.read().await.id.clone();
    let session_id = Uuid::new_v4().to_string();

    let session = Session::new(payload.node_id.clone());
    state
        .sessions
        .write()
        .await
        .insert(session_id.clone(), session);

    let connection_address = addr.ip().to_string();
    let remote_node_id = payload.node_id.clone();

    let mut peer = Peer::new(connection_address.clone(), payload.port);
    peer.id = remote_node_id.clone();
    peer.hostname = Some(payload.hostname.clone());
    peer.connected = true;
    peer.session_id = Some(session_id.clone());
    peer.health_check_failures = 0;

    {
        let mut peers = state.peers.write().await;
        if let Some(existing_peer) = peers.get_mut(&remote_node_id) {
            existing_peer.address = connection_address.clone();
            existing_peer.port = payload.port;
            existing_peer.hostname = Some(payload.hostname.clone());
            existing_peer.connected = true;
            existing_peer.session_id = Some(session_id.clone());
            existing_peer.last_seen = Utc::now();
            existing_peer.health_check_failures = 0;
        } else {
            peers.insert(peer.id.clone(), peer);
        }
    }

    let new_peer_info = models::PeerInfo {
        address: connection_address.clone(),
        port: payload.port,
        hostname: Some(payload.hostname.clone()),
        node_id: Some(remote_node_id.clone()),
    };
    broadcast_new_peer(&state, &remote_node_id, &new_peer_info);

    let known_peers: Vec<models::PeerInfo> = state
        .peers
        .read()
        .await
        .values()
        .filter(|p| p.id != remote_node_id)
        .filter(|p| p.address.parse::<std::net::IpAddr>().is_ok())
        .map(|p| models::PeerInfo {
            address: p.address.clone(),
            port: p.port,
            hostname: p.hostname.clone(),
            node_id: Some(p.id.clone()),
        })
        .collect();

    info!(
        "Accepted handshake from {}:{} (session: {})",
        payload.address, payload.port, session_id
    );

    let node_state = state.node_state.read().await;
    let advertised_address = if node_state.address == "0.0.0.0" {
        node_state
            .service_addresses
            .first()
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string())
    } else {
        node_state.address.clone()
    };

    Json(HandshakeResponse {
        accepted: true,
        node_id: Some(node_id),
        hostname: Some(node_state.hostname.clone()),
        address: Some(advertised_address),
        port: Some(node_state.port),
        session_id: Some(session_id),
        known_peers: Some(known_peers),
    })
}
