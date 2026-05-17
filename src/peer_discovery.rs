use std::time::Duration;

use chrono::Utc;
use tracing::{info, warn};

use crate::handlers;
use crate::models::{
    AppState, HandshakeRequest, HandshakeResponse, Session, StatusResponse,
};

const HEALTH_CHECK_FAILURE_THRESHOLD: u32 = 3;

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

        let connected_peers: Vec<(String, String, u16)> = {
            let peers = state.peers.read().await;
            peers
                .values()
                .filter(|p| p.connected)
                .map(|p| (p.id.clone(), p.address.clone(), p.port))
                .collect()
        };

        for (peer_id, addr, port) in &connected_peers {
            match state
                .http_client
                .get(format!("http://{}:{}/api/status", addr, port))
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(status) = resp.json::<StatusResponse>().await {
                        if status.node.id != *peer_id {
                            warn!(
                                "Health check: peer at {}:{} has different node_id (expected: {}, got: {}), replacing stale entry",
                                addr, port,
                                &peer_id[..8.min(peer_id.len())],
                                &status.node.id[..8.min(status.node.id.len())]
                            );
                            let mut peers = state.peers.write().await;
                            if let Some(stale) = peers.remove(peer_id) {
                                if let Some(sid) = stale.session_id {
                                    let mut sessions = state.sessions.write().await;
                                    sessions.remove(&sid);
                                }
                            }
                            if !peers.contains_key(&status.node.id) {
                                let mut new_peer = crate::models::Peer::new(addr.clone(), *port);
                                new_peer.id = status.node.id.clone();
                                new_peer.hostname = Some(status.node.hostname.clone());
                                new_peer.connected = true;
                                peers.insert(status.node.id.clone(), new_peer);
                            }
                            continue;
                        }
                    }
                    let mut peers = state.peers.write().await;
                    if let Some(peer) = peers.get_mut(peer_id) {
                        peer.last_seen = Utc::now();
                        peer.health_check_failures = 0;
                    }
                }
                Err(_) => {
                    let mut peers = state.peers.write().await;
                    if let Some(peer) = peers.get_mut(peer_id) {
                        peer.health_check_failures += 1;
                        if peer.health_check_failures >= HEALTH_CHECK_FAILURE_THRESHOLD {
                            let session_id = peer.session_id.take();
                            peer.connected = false;
                            warn!(
                                "Peer {}:{} (id: {}) failed {} health checks, disconnecting",
                                addr, port, &peer_id[..8.min(peer_id.len())],
                                peer.health_check_failures
                            );
                            if let Some(sid) = session_id {
                                let mut sessions = state.sessions.write().await;
                                sessions.remove(&sid);
                            }
                        }
                    }
                }
            }
        }

        let peers_to_try: Vec<(String, String, u16)> = {
            let peers = state.peers.read().await;
            peers
                .values()
                .filter(|p| !p.connected)
                .map(|p| (p.id.clone(), p.address.clone(), p.port))
                .collect()
        };

        for (peer_id, addr, port) in peers_to_try {
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
                            let remote_id = handshake
                                .node_id
                                .clone()
                                .unwrap_or_default();

                            let effective_id = if !remote_id.is_empty() && remote_id != peer_id {
                                warn!(
                                    "Reconnect: peer at {}:{} has different node_id (expected: {}, got: {}), replacing stale entry",
                                    addr, port,
                                    &peer_id[..8.min(peer_id.len())],
                                    &remote_id[..8.min(remote_id.len())]
                                );
                                let mut peers = state.peers.write().await;
                                if let Some(stale) = peers.remove(&peer_id) {
                                    if let Some(sid) = stale.session_id {
                                        let mut sessions = state.sessions.write().await;
                                        sessions.remove(&sid);
                                    }
                                }
                                remote_id.clone()
                            } else {
                                peer_id.clone()
                            };

                            let known_to_exchange = handshake.known_peers.clone();

                            let mut peers = state.peers.write().await;
                            if let Some(peer) = peers.get_mut(&effective_id) {
                                peer.connected = true;
                                peer.session_id = handshake.session_id.clone();
                                peer.health_check_failures = 0;
                                if let Some(hostname) = handshake.hostname {
                                    peer.hostname = Some(hostname);
                                }
                            } else {
                                let mut new_peer = crate::models::Peer::new(addr.clone(), port);
                                new_peer.id = effective_id.clone();
                                new_peer.connected = true;
                                new_peer.session_id = handshake.session_id.clone();
                                new_peer.hostname = handshake.hostname.clone();
                                peers.insert(effective_id.clone(), new_peer);
                            }
                            if let Some(session_id) = handshake.session_id {
                                let mut sessions = state.sessions.write().await;
                                sessions.insert(session_id, Session::new(effective_id.clone()));
                            }
                            info!("Reconnected to peer {}:{}", addr, port);

                            if let Some(kp) = known_to_exchange {
                                let state = state.clone();
                                let addr_clone = addr.clone();
                                tokio::spawn(async move {
                                    handlers::connect_to_unknown_peers(&state, kp, &addr_clone, port).await;
                                });
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }
}
