use std::sync::Arc;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use futures_util::StreamExt;
use iroh_gossip::api::Event;
use k2_core::K2Marketplace;
use tokio::sync::mpsc;
use crate::state::{AppState, Offer, TopicPeerEntry, WsEvent};

/// GET /api/broadcast-delay
pub async fn get_broadcast_delay() -> Json<serde_json::Value> {
    Json(json!({ "delay": K2Marketplace::get_broadcast_delay() }))
}

#[derive(Deserialize)]
pub struct JoinTopicBody {
    pub topic: String,
    pub action: String,
}

/// POST /api/topics/join
pub async fn join_topic(
    State(state): State<Arc<AppState>>,
    Json(body): Json<JoinTopicBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.clone().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?
    };
    let topic_id = K2Marketplace::topic_to_id(&body.topic);
    node.subscribe_topic_with_discovery(topic_id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to join topic: {}", e)))?;

    // Announce node mình lên tracker (với endpoint_addr đầy đủ để peers dial được)
    let node_id = node.my_id();
    let endpoint_addr_json = serde_json::to_value(node.endpoint.addr()).unwrap_or(serde_json::Value::Null);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    {
        let mut tracker = state.tracker_store.write().await;
        let entries = tracker.entry(body.topic.clone()).or_default();
        entries.retain(|e| e.node_id != node_id);
        entries.push(TopicPeerEntry { node_id, endpoint_addr: endpoint_addr_json, announced_at: now });
    }

    Ok(Json(json!({ "status": "joined", "topic": body.topic })))
}

#[derive(Deserialize)]
pub struct BroadcastOfferBody {
    pub topic: String,
    pub form_data: serde_json::Value,
    pub session_id: Option<String>,
}

/// POST /api/topics/broadcast
pub async fn broadcast_offer(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BroadcastOfferBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let my_node_id = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let node = guard.as_ref().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?;
        node.my_id()
    };

    // Dùng session_id từ frontend nếu có, fallback về node_id
    let sender_id = body.session_id.unwrap_or_else(|| my_node_id.clone());

    let payload = json!({
        "sender_node_id": sender_id,
        "message_type": "offer",
        "topic": body.topic,
        "form_data": body.form_data,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });

    let message = serde_json::to_vec(&payload).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sender = {
        let senders = state.topic_senders.read().await;
        senders.get(&body.topic).cloned()
    };

    match sender {
        Some(tx) => {
            tx.send(message).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Channel send failed: {}", e)))?;
            let offer_id = K2Marketplace::generate_id();
            Ok(Json(json!({ "status": "broadcast", "offer_id": offer_id })))
        }
        None => Err((StatusCode::BAD_REQUEST, format!("Not listening on topic: {}. Call start_listening first.", body.topic))),
    }
}

#[derive(Deserialize)]
pub struct SendInterestBody {
    pub topic: String,
    pub seller_node_id: String,
    pub form_data: serde_json::Value,
    pub session_id: Option<String>,
}

/// POST /api/topics/interest
pub async fn send_interest(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SendInterestBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let my_node_id = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let node = guard.as_ref().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?;
        node.my_id()
    };

    let sender_id = body.session_id.unwrap_or_else(|| my_node_id.clone());

    let payload = json!({
        "sender_node_id": sender_id,
        "target_node_id": body.seller_node_id,
        "message_type": "interest",
        "topic": body.topic,
        "form_data": body.form_data,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });

    let message = serde_json::to_vec(&payload).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sender = {
        let senders = state.topic_senders.read().await;
        senders.get(&body.topic).cloned()
    };

    match sender {
        Some(tx) => {
            tx.send(message).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Channel send failed: {}", e)))?;
            Ok(Json(json!({ "status": "sent", "target": body.seller_node_id })))
        }
        None => Err((StatusCode::BAD_REQUEST, format!("Not listening on topic: {}. Call start_listening first.", body.topic))),
    }
}

#[derive(Deserialize)]
pub struct ListenOffersQuery {
    pub topic: String,
    pub timeout: Option<u64>,
}

/// GET /api/topics/offers?topic=X&timeout=Y — long-poll
pub async fn listen_offers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListenOffersQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.clone().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?
    };

    let timeout_secs = query.timeout.unwrap_or(10);
    let topic_id = K2Marketplace::topic_to_id(&query.topic);
    let gossip_topic = node.subscribe_topic(topic_id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (_sender, mut receiver) = gossip_topic.split();
    let mut received: Vec<serde_json::Value> = Vec::new();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            receiver.next()
        ).await {
            Ok(Some(Ok(Event::Received(msg)))) => {
                if let Ok(offer) = serde_json::from_slice::<serde_json::Value>(&msg.content) {
                    received.push(offer);
                }
            }
            Ok(None) => break,
            _ => continue,
        }
    }

    Ok(Json(json!(received)))
}

#[derive(Deserialize)]
pub struct StartListeningBody {
    pub topic: String,
}

/// POST /api/topics/listen — start background listener, emits WsEvents
pub async fn start_listening(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StartListeningBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Check if already listening
    {
        let senders = state.topic_senders.write().await;
        if senders.contains_key(&body.topic) {
            return Ok(Json(json!({ "status": "already_listening", "topic": body.topic })));
        }
    }

    let node = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.clone().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    fn hex_to_pubkey(hex_id: &str) -> Option<iroh::PublicKey> {
        hex::decode(hex_id).ok()
            .and_then(|b| b.try_into().ok())
            .and_then(|arr: [u8; 32]| iroh::PublicKey::from_bytes(&arr).ok())
    }

    // 1. Query tracker: lấy peers đã announce trong topic này
    let tracker_peers: Vec<iroh::PublicKey> = {
        let tracker = state.tracker_store.read().await;
        tracker.get(&body.topic)
            .map(|entries| {
                entries.iter()
                    .filter(|e| now.saturating_sub(e.announced_at) < 3600)
                    .filter_map(|e| hex_to_pubkey(&e.node_id))
                    .collect()
            })
            .unwrap_or_default()
    };

    // 2. Load contacts as additional bootstrap peers
    let contact_peers: Vec<iroh::PublicKey> = {
        let contacts_guard = state.contacts.read().await;
        if let Some(ref cb) = *contacts_guard {
            cb.list().await.unwrap_or_default()
                .into_iter()
                .filter_map(|c| hex_to_pubkey(&c.node_id))
                .collect()
        } else {
            vec![]
        }
    };

    // 3. Merge, deduplicate
    let mut seen = std::collections::HashSet::new();
    let peer_keys: Vec<iroh::PublicKey> = tracker_peers.into_iter()
        .chain(contact_peers)
        .filter(|pk| seen.insert(*pk.as_bytes()))
        .collect();

    println!("[K2] start_listening '{}' with {} bootstrap peers (tracker+contacts)", body.topic, peer_keys.len());

    let topic_id = K2Marketplace::topic_to_id(&body.topic);
    let gossip_topic = node.subscribe_topic_with_peers(topic_id, peer_keys).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (sender, mut receiver) = gossip_topic.split();
    let sender = std::sync::Arc::new(sender);

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Store sender channel
    {
        let mut senders = state.topic_senders.write().await;
        if senders.contains_key(&body.topic) {
            return Ok(Json(json!({ "status": "already_listening", "topic": body.topic })));
        }
        senders.insert(body.topic.clone(), out_tx);
    }

    // Spawn outgoing forwarder
    let s = sender.clone();
    let topic_for_sender = body.topic.clone();
    tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if let Err(e) = s.broadcast(msg.into()).await {
                eprintln!("[K2] Broadcast error on {}: {}", topic_for_sender, e);
            }
        }
    });

    // Spawn incoming listener → push to event_tx
    let event_tx = state.event_tx.clone();
    let topic_clone = body.topic.clone();
    tokio::spawn(async move {
        loop {
            match receiver.next().await {
                Some(Ok(event)) => {
                    match event {
                        Event::Received(msg) => {
                            if let Ok(offer) = serde_json::from_slice::<serde_json::Value>(&msg.content) {
                                let _ = event_tx.send(WsEvent::OfferReceived { payload: offer });
                            }
                        }
                        Event::NeighborUp(id) => {
                            let _ = event_tx.send(WsEvent::PeerConnected { node_id: id.to_string() });
                        }
                        Event::NeighborDown(id) => {
                            let _ = event_tx.send(WsEvent::PeerDisconnected { node_id: id.to_string() });
                        }
                        _ => {}
                    }
                }
                Some(Err(e)) => eprintln!("[K2] Listener error on {}: {}", topic_clone, e),
                None => break,
            }
        }
    });

    // Announce node mình lên tracker
    {
        let node_id = node.my_id();
        let mut tracker = state.tracker_store.write().await;
        let entries = tracker.entry(body.topic.clone()).or_default();
        entries.retain(|e| e.node_id != node_id);
        entries.push(TopicPeerEntry { node_id, endpoint_addr: serde_json::Value::Null, announced_at: now });
    }

    Ok(Json(json!({ "status": "started", "topic": body.topic })))
}

// ── Web Matching Engine ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PostOfferBody {
    pub topic: String,
    pub action: String,   // "buy" | "sell" | "exchange"
    pub session_id: String,
    pub form_data: serde_json::Value,
}

/// POST /api/offers — Đăng offer lên server, tự match buy↔sell
pub async fn post_offer(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PostOfferBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let offer_id = K2Marketplace::generate_id();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let new_offer = Offer {
        offer_id: offer_id.clone(),
        session_id: body.session_id.clone(),
        topic: body.topic.clone(),
        action: body.action.clone(),
        form_data: body.form_data.clone(),
        timestamp,
    };

    // Tìm match trước khi lưu
    let matched = {
        let store = state.offer_store.read().await;
        let opposite = match body.action.as_str() {
            "buy" => "sell",
            "sell" => "buy",
            _ => "",
        };
        store.iter().find(|o| {
            o.topic == body.topic
                && o.action == opposite
                && o.session_id != body.session_id
        }).cloned()
    };

    // Lưu offer vào store
    {
        let mut store = state.offer_store.write().await;
        // Xóa offer cũ của cùng session+topic nếu có
        store.retain(|o| !(o.session_id == body.session_id && o.topic == body.topic));
        store.push(new_offer.clone());
    }

    if let Some(m) = matched {
        // Notify cả 2 qua WebSocket
        let match_payload = json!({
            "offer_a": {
                "offer_id": m.offer_id,
                "session_id": m.session_id,
                "action": m.action,
                "form_data": m.form_data,
            },
            "offer_b": {
                "offer_id": offer_id,
                "session_id": body.session_id,
                "action": body.action,
                "form_data": body.form_data,
            },
            "topic": body.topic,
            "timestamp": timestamp,
        });
        let _ = state.event_tx.send(WsEvent::OfferMatched { payload: match_payload.clone() });

        return Ok(Json(json!({
            "status": "matched",
            "offer_id": offer_id,
            "match": match_payload,
        })));
    }

    Ok(Json(json!({ "status": "waiting", "offer_id": offer_id })))
}

#[derive(Deserialize)]
pub struct GetOffersQuery {
    pub topic: Option<String>,
}

/// GET /api/offers — Lấy danh sách offers đang chờ
pub async fn get_offers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetOffersQuery>,
) -> Json<serde_json::Value> {
    let store = state.offer_store.read().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Lọc offers còn hiệu lực (< 5 phút)
    let offers: Vec<&Offer> = store.iter()
        .filter(|o| {
            let fresh = now.saturating_sub(o.timestamp) < 300;
            let topic_match = query.topic.as_ref()
                .map(|t| &o.topic == t)
                .unwrap_or(true);
            fresh && topic_match
        })
        .collect();

    Json(json!(offers))
}
