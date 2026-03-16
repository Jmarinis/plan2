use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

/// Unique identifier for each peer node
type PeerId = String;

/// Generate a deterministic peer ID based on address and port
fn generate_peer_id(address: &str, port: u16) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", address, port).as_bytes());
    hex::encode(hasher.finalize())
}

/// Represents a known peer in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: PeerId,
    pub address: String,
    pub port: u16,
    pub hostname: Option<String>,
    pub connected: bool,
    pub last_seen: DateTime<Utc>,
    pub session_id: Option<String>,
}

impl Peer {
    pub fn new(address: String, port: u16) -> Self {
        Self {
            id: generate_peer_id(&address, port),
            address,
            port,
            hostname: None,
            connected: false,
            last_seen: Utc::now(),
            session_id: None,
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.address, self.port)
    }
}

/// Represents an active session between peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub peer_id: PeerId,
    pub established_at: DateTime<Utc>,
    pub peers_exchanged: bool,
}

impl Session {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            peer_id,
            established_at: Utc::now(),
            peers_exchanged: false,
        }
    }
}

/// Current state of the local node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub id: PeerId,
    pub address: String,
    pub port: u16,
    pub hostname: String,
    pub service_addresses: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub uptime_seconds: u64,
}

impl NodeState {
    pub fn new(address: String, port: u16, hostname: String) -> Self {
        // Get all local IP addresses for display
        let service_addresses: Vec<String> = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .iter()
            .filter(|iface| {
                // Skip loopback and link-local addresses
                !iface.ip().is_loopback() && !iface.ip().is_multicast()
            })
            .map(|iface| iface.ip().to_string())
            .collect();
        
        Self {
            id: Uuid::new_v4().to_string(),
            address,
            port,
            hostname,
            service_addresses,
            started_at: Utc::now(),
            uptime_seconds: 0,
        }
    }
}

/// Handshake request sent when connecting to a peer
#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub node_id: PeerId,
    pub address: String,
    pub port: u16,
    pub hostname: String,
}

/// Handshake response from a peer
#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub accepted: bool,
    pub node_id: Option<PeerId>,
    pub hostname: Option<String>,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub session_id: Option<String>,
    pub known_peers: Option<Vec<PeerInfo>>,
}

/// Disconnect notification sent to peers
#[derive(Debug, Serialize, Deserialize)]
pub struct DisconnectRequest {
    pub node_id: PeerId,
    pub session_id: String,
    pub reason: String,
}

/// Disconnect response
#[derive(Debug, Serialize, Deserialize)]
pub struct DisconnectResponse {
    pub accepted: bool,
}

/// Simplified peer info for exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub address: String,
    pub port: u16,
    pub hostname: Option<String>,
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub node_state: Arc<RwLock<NodeState>>,
    pub peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
    pub sessions: Arc<RwLock<HashMap<String, Session>>>,
    pub http_client: Client,
}

impl AppState {
    pub fn new(address: String, port: u16, hostname: String) -> Self {
        Self {
            node_state: Arc::new(RwLock::new(NodeState::new(address, port, hostname))),
            peers: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            http_client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }
}

/// Status response for the web interface
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub node: NodeState,
    pub connected_peers: Vec<Peer>,
    pub known_peers: Vec<Peer>,
    pub active_sessions: Vec<Session>,
}

/// API response for adding a peer
#[derive(Debug, Serialize, Deserialize)]
pub struct AddPeerRequest {
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Serialize)]
pub struct AddPeerResponse {
    pub success: bool,
    pub peer: Option<Peer>,
    pub message: String,
}

/// API request for removing a peer
#[derive(Debug, Serialize, Deserialize)]
pub struct RemovePeerRequest {
    pub peer_id: PeerId,
    pub notify_peer: bool,
}

#[derive(Debug, Serialize)]
pub struct RemovePeerResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// Main web interface showing node status
async fn index_handler() -> Html<&'static str> {
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
                <input type="number" id="peer-port" placeholder="Port" required style="max-width:100px">
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
            document.getElementById('node-port').textContent = data.node.port;
            document.getElementById('node-uptime').textContent = data.node.uptime_seconds + 's';
            
            // Display service addresses
            const addresses = data.node.service_addresses || [];
            document.getElementById('service-addresses').innerHTML = addresses
                .map(addr => `<span style="display: inline-block; background: #0f3460; padding: 5px 10px; border-radius: 4px; margin: 3px;">${addr}:${data.node.port}</span>`)
                .join('') || '<span style="color: #888;">No addresses available</span>';

            const connectedBody = document.querySelector('#connected-peers tbody');
            connectedBody.innerHTML = data.connected_peers.map(p =>
                `<tr><td>${p.id.slice(0,16)}...</td><td>${p.hostname || '-'}</td><td>${p.address}</td><td>${p.port}</td>
                <td class="status-connected">${p.session_id ? p.session_id.slice(0,8)+'...' : '-'}</td>
                <td>${new Date(p.last_seen).toLocaleTimeString()}</td>
                <td><button onclick="removePeer('${p.id}')" style="background:#ff6b6b;color:white;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;">Disconnect</button></td></tr>`
            ).join('') || '<tr><td colspan="7">No connected peers</td></tr>';
            document.getElementById('connected-count').textContent = data.connected_peers.length;

            const knownBody = document.querySelector('#known-peers tbody');
            knownBody.innerHTML = data.known_peers.map(p => {
                const actionBtn = p.connected
                    ? `<button onclick="removePeer('${p.id}')" style="background:#ff6b6b;color:white;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;">Remove</button>`
                    : `<button onclick="connectPeer('${p.id}')" style="background:#00d9ff;color:#1a1a2e;padding:4px 8px;border:none;border-radius:3px;cursor:pointer;font-weight:bold;">Connect</button>`;
                return `<tr><td>${p.id.slice(0,16)}...</td><td>${p.hostname || '-'}</td><td>${p.address}</td><td>${p.port}</td>
                <td class="${p.connected ? 'status-connected' : 'status-disconnected'}">${p.connected ? 'Connected' : 'Disconnected'}</td>
                <td>${new Date(p.last_seen).toLocaleTimeString()}</td>
                <td>${actionBtn}</td></tr>`;
            }).join('') || '<tr><td colspan="7">No known peers</td></tr>';
            document.getElementById('known-count').textContent = data.known_peers.length;
        }

        async function removePeer(peerId) {
            if (!confirm('Remove this peer from the local list?')) {
                return;
            }
            await fetch('/api/peers/remove', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({peer_id: peerId})
            });
            loadStatus();
        }

        async function connectPeer(peerId) {
            await fetch('/api/peers/connect', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({peer_id: peerId})
            });
            loadStatus();
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

/// Get node status API
async fn status_handler(State(state): State<AppState>) -> Json<StatusResponse> {
    let node_state = state.node_state.read().await.clone();
    let peers = state.peers.read().await;
    let sessions = state.sessions.read().await.clone();

    let connected_peers: Vec<Peer> = peers.values().filter(|p| p.connected).cloned().collect();
    let known_peers: Vec<Peer> = peers.values().cloned().collect();
    let active_sessions: Vec<Session> = sessions.values().cloned().collect();

    Json(StatusResponse {
        node: node_state,
        connected_peers,
        known_peers,
        active_sessions,
    })
}

/// Add a new peer to connect to
async fn add_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<AddPeerRequest>,
) -> impl IntoResponse {
    // Check if peer already exists
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

    // Determine our advertised address - use first service address if bound to 0.0.0.0
    let node_state = state.node_state.read().await;
    let our_address = if node_state.address == "0.0.0.0" {
        node_state.service_addresses.first()
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string())
    } else {
        node_state.address.clone()
    };
    let our_port = node_state.port;
    let our_hostname = node_state.hostname.clone();
    let our_node_id = node_state.id.clone();
    drop(node_state);

    // Try to establish connection
    match state.http_client
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
                    // Store the hostname and address from the handshake response
                    peer.hostname = handshake.hostname;
                    // Use the peer's advertised address if provided
                    if let Some(addr) = handshake.address {
                        peer.address = addr;
                    }
                    if let Some(port) = handshake.port {
                        peer.port = port;
                    }

                    // Store the session
                    if let Some(session_id) = &peer.session_id {
                        let mut sessions = state.sessions.write().await;
                        sessions.insert(session_id.clone(), Session::new(peer.id.clone()));
                    }

                    // Add known peers from handshake
                    if let Some(known_peers) = handshake.known_peers {
                        let mut peers = state.peers.write().await;
                        for kp in known_peers {
                            if kp.address != payload.address || kp.port != payload.port {
                                let peer_id = generate_peer_id(&kp.address, kp.port);
                                peers.entry(peer_id).or_insert_with(|| {
                                    let mut p = Peer::new(kp.address.clone(), kp.port);
                                    p.hostname = kp.hostname;
                                    p.last_seen = Utc::now();
                                    p
                                });
                            }
                        }
                    }

                    info!("Connected to peer {}:{}", peer.address, peer.port);
                }
            }
        }
        Err(e) => {
            warn!("Failed to connect to peer {}:{} - {}", peer.address, peer.port, e);
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

/// Remove a peer locally (disconnect without notifying)
async fn remove_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<RemovePeerRequest>,
) -> impl IntoResponse {
    let mut peers = state.peers.write().await;

    if let Some(peer) = peers.get(&payload.peer_id) {
        let peer_clone = peer.clone();

        // Remove the peer locally
        peers.remove(&payload.peer_id);

        // Remove associated session
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

/// Attempt to connect to a known peer
async fn connect_peer_handler(
    State(state): State<AppState>,
    Json(payload): Json<RemovePeerRequest>,
) -> impl IntoResponse {
    let peers = state.peers.read().await;
    
    if let Some(peer) = peers.get(&payload.peer_id) {
        let peer_clone = peer.clone();
        drop(peers);
        
        // Try to establish connection
        let node_state = state.node_state.read().await;
        let our_address = if node_state.address == "0.0.0.0" {
            node_state.service_addresses.first()
                .cloned()
                .unwrap_or_else(|| "127.0.0.1".to_string())
        } else {
            node_state.address.clone()
        };
        let our_port = node_state.port;
        let our_hostname = node_state.hostname.clone();
        let our_node_id = node_state.id.clone();
        drop(node_state);
        
        match state.http_client
            .post(format!("http://{}:{}/api/handshake", peer_clone.address, peer_clone.port))
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
                        let mut peers = state.peers.write().await;
                        if let Some(p) = peers.get_mut(&payload.peer_id) {
                            p.connected = true;
                            p.session_id = handshake.session_id.clone();
                            p.hostname = handshake.hostname;
                            if let Some(addr) = handshake.address {
                                p.address = addr;
                            }
                            if let Some(port) = handshake.port {
                                p.port = port;
                            }
                            p.last_seen = Utc::now();
                        }
                        
                        if let Some(session_id) = handshake.session_id {
                            let mut sessions = state.sessions.write().await;
                            sessions.insert(session_id, Session::new(String::new()));
                        }
                        
                        info!("Connected to peer {}:{}", peer_clone.address, peer_clone.port);
                        
                        return (
                            StatusCode::OK,
                            Json(RemovePeerResponse {
                                success: true,
                                message: format!("Connected to peer {}", payload.peer_id),
                            }),
                        );
                    }
                }
            }
            Err(e) => {
                warn!("Failed to connect to peer {}:{} - {}", peer_clone.address, peer_clone.port, e);
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

/// Handle incoming disconnect requests from peers
async fn disconnect_handler(
    State(state): State<AppState>,
    Json(payload): Json<DisconnectRequest>,
) -> Json<DisconnectResponse> {
    // Remove the peer that is disconnecting
    let mut peers = state.peers.write().await;
    let mut removed = false;
    
    peers.retain(|_, peer| {
        if peer.session_id.as_ref() == Some(&payload.session_id) {
            removed = true;
            info!("Peer {}:{} disconnected: {}", peer.address, peer.port, payload.reason);
            false
        } else {
            true
        }
    });
    
    // Remove the session
    if removed {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&payload.session_id);
    }
    
    Json(DisconnectResponse { accepted: removed })
}

/// Handle incoming handshake requests from peers
async fn handshake_handler(
    State(state): State<AppState>,
    Json(payload): Json<HandshakeRequest>,
) -> Json<HandshakeResponse> {
    let node_id = state.node_state.read().await.id.clone();
    let session_id = Uuid::new_v4().to_string();

    // Create session
    let session = Session::new(payload.node_id.clone());
    state.sessions.write().await.insert(session_id.clone(), session);

    // Add or update peer
    let mut peer = Peer::new(payload.address.clone(), payload.port);
    peer.hostname = Some(payload.hostname.clone());
    peer.connected = true;
    peer.session_id = Some(session_id.clone());

    {
        let mut peers = state.peers.write().await;
        // Check if peer exists by address
        let mut existing_id = None;
        for (id, p) in peers.iter() {
            if p.address == payload.address && p.port == payload.port {
                existing_id = Some(id.clone());
                break;
            }
        }
        if let Some(id) = existing_id {
            // Update existing peer with new hostname and connection info
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

    // Gather known peers to share
    let known_peers: Vec<PeerInfo> = state.peers.read().await.values()
        .filter(|p| !(p.address == payload.address && p.port == payload.port))
        .map(|p| PeerInfo {
            address: p.address.clone(),
            port: p.port,
            hostname: p.hostname.clone(),
        })
        .collect();

    info!("Accepted handshake from {}:{} (session: {})", payload.address, payload.port, session_id);

    let node_state = state.node_state.read().await;
    // Use a routable address instead of 0.0.0.0
    let advertised_address = if node_state.address == "0.0.0.0" {
        node_state.service_addresses.first()
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

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("p2p_node=info".parse().unwrap()),
        )
        .init();

    // Configuration
    // Use 0.0.0.0 to accept connections from any interface (required for remote peers)
    // Use 127.0.0.1 for localhost-only access
    let address = std::env::var("P2P_ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("P2P_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);
    // Get hostname from env or system
    let hostname = std::env::var("P2P_HOSTNAME")
        .unwrap_or_else(|_| {
            hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        });

    info!("Starting P2P node on {}:{} (hostname: {})", address, port, hostname);

    // Create shared state
    let state = AppState::new(address.clone(), port, hostname);
    let node_id = state.node_state.read().await.id.clone();
    info!("Node ID: {}", node_id);

    // Build router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/status", get(status_handler))
        .route("/api/peers", post(add_peer_handler))
        .route("/api/peers/remove", post(remove_peer_handler))
        .route("/api/peers/connect", post(connect_peer_handler))
        .route("/api/handshake", post(handshake_handler))
        .route("/api/disconnect", post(disconnect_handler))
        .with_state(state.clone());

    // Start periodic peer discovery and status checks
    let discovery_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;

            // Update uptime
            {
                let mut node = discovery_state.node_state.write().await;
                node.uptime_seconds = Utc::now().signed_duration_since(node.started_at)
                    .num_seconds() as u64;
            }

            // Check status of connected peers
            let connected_peers: Vec<(String, u16, String)> = {
                let peers = discovery_state.peers.read().await;
                peers.values()
                    .filter(|p| p.connected)
                    .map(|p| (p.address.clone(), p.port, p.session_id.clone().unwrap_or_default()))
                    .collect()
            };

            for (addr, port, session_id) in connected_peers {
                // Try to get status from peer to verify connection
                match discovery_state.http_client
                    .get(format!("http://{}:{}/api/status", addr, port))
                    .send()
                    .await
                {
                    Ok(_) => {
                        // Peer is still alive, update last_seen
                        let mut peers = discovery_state.peers.write().await;
                        if let Some(peer) = peers.values_mut().find(|p| p.address == addr && p.port == port) {
                            peer.last_seen = Utc::now();
                        }
                    }
                    Err(_) => {
                        // Peer is not responding, mark as disconnected
                        let mut peers = discovery_state.peers.write().await;
                        if let Some(peer) = peers.values_mut().find(|p| p.address == addr && p.port == port) {
                            peer.connected = false;
                            peer.session_id = None;
                            warn!("Peer {}:{} is no longer responding", addr, port);
                        }
                        // Remove session
                        if !session_id.is_empty() {
                            let mut sessions = discovery_state.sessions.write().await;
                            sessions.remove(&session_id);
                        }
                    }
                }
            }

            // Try to reconnect to disconnected peers
            let peers_to_try: Vec<(String, u16)> = {
                let peers = discovery_state.peers.read().await;
                peers.values()
                    .filter(|p| !p.connected)
                    .map(|p| (p.address.clone(), p.port))
                    .collect()
            };

            for (addr, port) in peers_to_try {
                // Determine our advertised address
                let node_state = discovery_state.node_state.read().await;
                let our_address = if node_state.address == "0.0.0.0" {
                    node_state.service_addresses.first()
                        .cloned()
                        .unwrap_or_else(|| "127.0.0.1".to_string())
                } else {
                    node_state.address.clone()
                };
                let our_port = node_state.port;
                let our_hostname = node_state.hostname.clone();
                let our_node_id = node_state.id.clone();
                drop(node_state);

                match discovery_state.http_client
                    .post(format!("http://{}:{}/api/handshake", addr, port))
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
                                let mut peers = discovery_state.peers.write().await;
                                for peer in peers.values_mut() {
                                    if peer.address == addr && peer.port == port {
                                        peer.connected = true;
                                        peer.session_id = handshake.session_id.clone();
                                        // Update from handshake response
                                        if let Some(hostname) = handshake.hostname {
                                            peer.hostname = Some(hostname);
                                        }
                                        if let Some(address) = handshake.address {
                                            peer.address = address;
                                        }
                                        if let Some(port) = handshake.port {
                                            peer.port = port;
                                        }
                                        break;
                                    }
                                }
                                if let Some(session_id) = handshake.session_id {
                                    let mut sessions = discovery_state.sessions.write().await;
                                    sessions.insert(session_id, Session::new(String::new()));
                                }
                                info!("Reconnected to peer {}:{}", addr, port);
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    });

    // Start HTTP server
    let addr: SocketAddr = format!("{}:{}", address, port).parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("Web interface available at http://{}", addr);
    
    axum::serve(listener, app).await.unwrap();
}
