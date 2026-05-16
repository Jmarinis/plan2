use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use uuid::Uuid;

pub type PeerId = String;

pub fn generate_peer_id(address: &str, port: u16) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", address, port).as_bytes());
    hex::encode(hasher.finalize())
}

fn load_or_create_node_id() -> String {
    let path = std::path::Path::new("node_id");
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(path) {
            let id = data.trim().to_string();
            if !id.is_empty() {
                return id;
            }
        }
    }
    let id = Uuid::new_v4().to_string();
    let _ = std::fs::write(path, &id);
    id
}

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
        let service_addresses: Vec<String> = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .iter()
            .filter(|iface| !iface.ip().is_loopback() && !iface.ip().is_multicast())
            .map(|iface| iface.ip().to_string())
            .collect();

        Self {
            id: load_or_create_node_id(),
            address,
            port,
            hostname,
            service_addresses,
            started_at: Utc::now(),
            uptime_seconds: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub node_id: PeerId,
    pub address: String,
    pub port: u16,
    pub hostname: String,
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct DisconnectRequest {
    pub node_id: PeerId,
    pub session_id: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisconnectResponse {
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub address: String,
    pub port: u16,
    pub hostname: Option<String>,
    pub node_id: Option<PeerId>,
}

#[derive(Clone)]
pub struct AppState {
    pub node_state: Arc<RwLock<NodeState>>,
    pub peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
    pub sessions: Arc<RwLock<HashMap<String, Session>>>,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(address: String, port: u16, hostname: String) -> Self {
        Self {
            node_state: Arc::new(RwLock::new(NodeState::new(address, port, hostname))),
            peers: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub node: NodeState,
    pub connected_peers: Vec<Peer>,
    pub known_peers: Vec<Peer>,
    pub active_sessions: Vec<Session>,
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct RemovePeerRequest {
    pub peer_id: PeerId,
    pub notify_peer: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RemovePeerResponse {
    pub success: bool,
    pub message: String,
}
