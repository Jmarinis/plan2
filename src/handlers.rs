use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use chrono::Utc;
use std::net::SocketAddr;
use tracing::{info, warn};
use uuid::Uuid;

use crate::models::{
    self, AddPeerRequest, AddPeerResponse, AppState, DisconnectRequest, DisconnectResponse,
    HandshakeRequest, HandshakeResponse, Peer, RemovePeerRequest, RemovePeerResponse, Session,
    StatusResponse,
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
        <button class="refresh" onclick="location.reload()">↻ Refresh</button>
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
                <thead><tr><th>Peer ID</th><th>Hostname</th><th>Address</th><th>Port</th><th>Session</th><th>Last Seen</th><th>Actions</th></tr></thead>
                <tbody></tbody>
            </table>
        </div>

        <div class="card">
            <h2>📋 Known Peers (<span id="known-count">0</span>)</h2>
            <table id="known-peers">
                <thead><tr><th>Peer ID</th><th>Hostname</th><th>Address</th><th>Port</th><th>Status</th><th>Last Seen</th><th>Actions</th></tr></thead>
                <tbody></tbody>
            </table>
        </div>
    </div>
    <script>
        async function loadStatus() {
            const res = await fetch('/api/status');
            const data = await res.json();

            document.getElementById('node-id').textContent = data.node.id.slice(0, 8) + '...';
            document.getElementById('node-hostname').textContent = data.node.hostname;
            document.title = `P2P Node - ${data.node.hostname}`;
            document.getElementById('node-port').textContent = data.node.port;
            document.getElementById('node-uptime').textContent = data.node.uptime_seconds + 's';
            
            const addresses = data.node.service_addresses || [];
            document.getElementById('service-addresses').innerHTML = addresses
                .map(addr => `<span style="display: inline-block; background: #0f3460; padding: 5px 10px; border-radius: 4px; margin: 3px;">${addr}:${data.node.port}</span>`)
                .join('') || '<span style="color: #888;">No addresses available</span>';

            const connectedBody = document.querySelector('#connected-peers tbody');
            connectedBody.innerHTML = data.connected_peers.map(p =>
                `<tr><td>${p.id.slice(0,16)}...</td><td>${p.hostname || '-'}</td><td>${p.address}</td><td>${p.port}</td>
                <td class="status-connected">${p.session_id ? p.session_id.slice(0,8)+'...' : '-'}</td>
                <td>${new Date(p.last_seen).toLocaleTimeString()}</td>
                <td><button onclick="disconnectPeer('${p.id}')" style="background:#ff6b6b;color:white;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;">Disconnect</button></td></tr>`
            ).join('') || '<tr><td colspan="7">No connected peers</td></tr>';
            document.getElementById('connected-count').textContent = data.connected_peers.length;

            const knownPeersOnly = data.known_peers.filter(p => !p.connected);
            const knownBody = document.querySelector('#known-peers tbody');
            knownBody.innerHTML = knownPeersOnly.map(p =>
                `<tr><td>${p.id.slice(0,16)}...</td><td>${p.hostname || '-'}</td><td>${p.address}</td><td>${p.port}</td>
                <td class="status-disconnected">Disconnected</td>
                <td>${new Date(p.last_seen).toLocaleTimeString()}</td>
                <td>
                    <button onclick="connectPeer('${p.id}')" style="background:#00d9ff;color:#1a1a2e;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;font-weight:bold;">Connect</button>
                    <button onclick="removePeer('${p.id}')" style="background:#ff6b6b;color:white;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;margin-left:5px;">Remove</button>
                </td></tr>`
            ).join('') || '<tr><td colspan="7">No known peers</td></tr>';
            document.getElementById('known-count').textContent = knownPeersOnly.length;
        }

        async function disconnectPeer(peerId) {
            console.log('Disconnecting peer:', peerId);
            try {
                const response = await fetch('/api/peers/disconnect', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({peer_id: peerId})
                });
                console.log('Disconnect response:', response.status);
                loadStatus();
            } catch (error) {
                console.error('Disconnect failed:', error);
            }
        }

        async function removePeer(peerId) {
            console.log('Removing peer:', peerId);
            try {
                const response = await fetch('/api/peers/remove', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({peer_id: peerId})
                });
                console.log('Remove response:', response.status);
                loadStatus();
            } catch (error) {
                console.error('Remove failed:', error);
            }
        }

        async function connectPeer(peerId) {
            console.log('Connecting to peer:', peerId);
            try {
                const response = await fetch('/api/peers/connect', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({peer_id: peerId})
                });
                console.log('Connect response:', response.status);
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

        loadStatus();
        setInterval(loadStatus, 5000);
    </script>
</body>
</html>"#,
    )
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
    {
        let peers = state.peers.read().await;
        for peer in peers.values() {
            if peer.address == payload.address && peer.port == payload.port {
                return (
                    StatusCode::OK,
                    Json(AddPeerResponse {
                        success: true,
                        peer: Some(peer.clone()),
                        message: "Peer already known".to_string(),
                    }),
                );
            }
        }
    }

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
            node_id: our_node_id,
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
                    peer.connected = true;
                    peer.session_id = handshake.session_id;
                    peer.hostname = handshake.hostname;

                    if let Some(session_id) = &peer.session_id {
                        let mut sessions = state.sessions.write().await;
                        sessions.insert(session_id.clone(), Session::new(peer.id.clone()));
                    }

                    if let Some(known_peers) = handshake.known_peers {
                        let mut peers = state.peers.write().await;
                        for kp in known_peers {
                            if kp.address != payload.address || kp.port != payload.port {
                                if kp.address.parse::<std::net::IpAddr>().is_ok() {
                                    let peer_id = models::generate_peer_id(&kp.address, kp.port);
                                    peers.entry(peer_id).or_insert_with(|| {
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
                    }

                    info!("Connected to peer {}:{}", peer.address, peer.port);
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
    peers.insert(peer_id.clone(), peer.clone());

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

    if let Some(peer) = peers.get(&payload.peer_id) {
        let peer_clone = peer.clone();

        if peer_clone.connected {
            if let Some(session_id) = &peer_clone.session_id {
                let _ = state
                    .http_client
                    .post(format!(
                        "http://{}:{}/api/disconnect-session",
                        peer_clone.address, peer_clone.port
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

        peers.remove(&payload.peer_id);

        if let Some(session_id) = &peer_clone.session_id {
            let mut sessions = state.sessions.write().await;
            sessions.remove(session_id);
        }

        info!("Removed peer {}:{}", peer_clone.address, peer_clone.port);

        (
            StatusCode::OK,
            Json(RemovePeerResponse {
                success: true,
                message: format!("Peer {} removed", payload.peer_id),
            }),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(RemovePeerResponse {
                success: false,
                message: "Peer not found".to_string(),
            }),
        )
    }
}

pub async fn disconnect_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<RemovePeerRequest>,
) -> impl IntoResponse {
    let mut peers = state.peers.write().await;

    if let Some(peer) = peers.get(&payload.peer_id) {
        let peer_clone = peer.clone();
        info!(
            "Disconnect request for peer {} (connected: {})",
            payload.peer_id, peer_clone.connected
        );

        if peer_clone.connected {
            if let Some(session_id) = &peer_clone.session_id {
                info!(
                    "Notifying peer {}:{} to disconnect",
                    peer_clone.address, peer_clone.port
                );
                let _ = state
                    .http_client
                    .post(format!(
                        "http://{}:{}/api/disconnect-session",
                        peer_clone.address, peer_clone.port
                    ))
                    .json(&DisconnectRequest {
                        node_id: state.node_state.read().await.id.clone(),
                        session_id: session_id.clone(),
                        reason: "Peer initiated disconnect".to_string(),
                    })
                    .send()
                    .await;
            }
        }

        if let Some(peer) = peers.get_mut(&payload.peer_id) {
            peer.connected = false;
            peer.session_id = None;
            info!("Peer {} marked as disconnected", payload.peer_id);
        }

        if let Some(session_id) = &peer_clone.session_id {
            let mut sessions = state.sessions.write().await;
            sessions.remove(session_id);
        }

        info!(
            "Disconnected from peer {}:{}",
            peer_clone.address, peer_clone.port
        );

        (
            StatusCode::OK,
            Json(RemovePeerResponse {
                success: true,
                message: format!("Disconnected from peer {}", payload.peer_id),
            }),
        )
    } else {
        info!("Peer {} not found for disconnect", payload.peer_id);
        (
            StatusCode::NOT_FOUND,
            Json(RemovePeerResponse {
                success: false,
                message: "Peer not found".to_string(),
            }),
        )
    }
}

pub async fn connect_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<RemovePeerRequest>,
) -> impl IntoResponse {
    let peers = state.peers.read().await;

    if let Some(peer) = peers.get(&payload.peer_id) {
        let peer_address = peer.address.clone();
        let peer_port = peer.port;
        let peer_connected = peer.connected;
        drop(peers);

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
                            let mut peers = state.peers.write().await;
                            if let Some(p) = peers.get_mut(&payload.peer_id) {
                                p.connected = true;
                                p.session_id = handshake.session_id.clone();
                                p.hostname = handshake.hostname;
                                p.last_seen = Utc::now();
                            }

                            if let Some(session_id) = handshake.session_id {
                                let mut sessions = state.sessions.write().await;
                                sessions.insert(session_id, Session::new(String::new()));
                            }

                            info!("Connected to peer {}:{}", peer_address, peer_port);

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
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(RemovePeerResponse {
                success: false,
                message: "Peer not found".to_string(),
            }),
        )
    }
}

pub async fn disconnect_session_handler(
    State(state): State<AppState>,
    Json(payload): Json<DisconnectRequest>,
) -> Json<DisconnectResponse> {
    let mut peers = state.peers.write().await;
    let mut disconnected = false;

    for peer in peers.values_mut() {
        if peer.session_id.as_ref() == Some(&payload.session_id) {
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
        if peer.session_id.as_ref() == Some(&payload.session_id) {
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

    let mut peer = Peer::new(connection_address.clone(), payload.port);
    peer.hostname = Some(payload.hostname.clone());
    peer.connected = true;
    peer.session_id = Some(session_id.clone());

    {
        let mut peers = state.peers.write().await;
        let mut existing_id = None;
        for (id, p) in peers.iter() {
            if p.address == payload.address && p.port == payload.port {
                existing_id = Some(id.clone());
                break;
            }
        }
        if let Some(id) = existing_id {
            if let Some(existing_peer) = peers.get_mut(&id) {
                existing_peer.hostname = Some(payload.hostname.clone());
                existing_peer.connected = true;
                existing_peer.session_id = Some(session_id.clone());
                existing_peer.last_seen = Utc::now();
            }
            peer.id = id.clone();
            peers.insert(id, peer);
        } else {
            peers.insert(peer.id.clone(), peer);
        }
    }

    let known_peers: Vec<models::PeerInfo> = state
        .peers
        .read()
        .await
        .values()
        .filter(|p| !(p.address == payload.address && p.port == payload.port))
        .filter(|p| p.address.parse::<std::net::IpAddr>().is_ok())
        .map(|p| models::PeerInfo {
            address: p.address.clone(),
            port: p.port,
            hostname: p.hostname.clone(),
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
