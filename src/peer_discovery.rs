use std::time::Duration;

use chrono::Utc;
use tracing::{info, warn};

use crate::models::{
    AppState, HandshakeRequest, HandshakeResponse, Session,
};

pub async fn start(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;

        {
            let mut node = state.node_state.write().await;
            node.uptime_seconds = Utc::now()
                .signed_duration_since(node.started_at)
                .num_seconds() as u64;
        }

        let connected_peers: Vec<(String, u16, String)> = {
            let peers = state.peers.read().await;
            peers
                .values()
                .filter(|p| p.connected)
                .map(|p| {
                    (
                        p.address.clone(),
                        p.port,
                        p.session_id.clone().unwrap_or_default(),
                    )
                })
                .collect()
        };

        for (addr, port, session_id) in connected_peers {
            match state
                .http_client
                .get(format!("http://{}:{}/api/status", addr, port))
                .send()
                .await
            {
                Ok(_) => {
                    let mut peers = state.peers.write().await;
                    if let Some(peer) = peers
                        .values_mut()
                        .find(|p| p.address == addr && p.port == port)
                    {
                        peer.last_seen = Utc::now();
                    }
                }
                Err(_) => {
                    let mut peers = state.peers.write().await;
                    if let Some(peer) = peers
                        .values_mut()
                        .find(|p| p.address == addr && p.port == port)
                    {
                        peer.connected = false;
                        peer.session_id = None;
                        warn!("Peer {}:{} is no longer responding", addr, port);
                    }
                    if !session_id.is_empty() {
                        let mut sessions = state.sessions.write().await;
                        sessions.remove(&session_id);
                    }
                }
            }
        }

        let peers_to_try: Vec<(String, u16)> = {
            let peers = state.peers.read().await;
            peers
                .values()
                .filter(|p| !p.connected)
                .map(|p| (p.address.clone(), p.port))
                .collect()
        };

        for (addr, port) in peers_to_try {
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
                            let mut peers = state.peers.write().await;
                            for peer in peers.values_mut() {
                                if peer.address == addr && peer.port == port {
                                    peer.connected = true;
                                    peer.session_id = handshake.session_id.clone();
                                    if let Some(hostname) = handshake.hostname {
                                        peer.hostname = Some(hostname);
                                    }
                                    break;
                                }
                            }
                            if let Some(session_id) = handshake.session_id {
                                let mut sessions = state.sessions.write().await;
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
}
