use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
use tauri::Manager;
use k2_core::{K2Node, Contact, ContactBook, K2Marketplace};
use qrcode::QrCode;
use qrcode::render::svg;
#[cfg(not(target_os = "android"))]
use directories::UserDirs;

/// App State - holds the K2Node instance and ContactBook
pub struct AppState {
    pub node: Mutex<Option<K2Node>>,
    pub contacts: Mutex<ContactBook>,
    pub contacts_path: Mutex<Option<PathBuf>>,
}

/// Initialize the K2 P2P Node
#[tauri::command]
async fn init_node(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<String, String> {
    let node = K2Node::new().await.map_err(|e| e.to_string())?;
    let node_id = node.my_id();
    
    // Shorten for display
    let short_id = if node_id.len() > 10 {
        format!("{}...", &node_id[..10])
    } else {
        node_id.clone()
    };
    
    // Store node in state
    {
        let mut state_node = state.node.lock().unwrap();
        *state_node = Some(node);
    }
    
    // Load contacts from app data directory
    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let contacts_file = app_data_dir.join("contacts.json");
        let loaded_contacts = ContactBook::load(&contacts_file).unwrap_or_default();
        
        let mut contacts = state.contacts.lock().unwrap();
        *contacts = loaded_contacts;
        
        let mut path = state.contacts_path.lock().unwrap();
        *path = Some(contacts_file);
    }
    
    Ok(short_id)
}

/// Get full NodeId for sharing
#[tauri::command]
async fn get_my_node_id(state: State<'_, AppState>) -> Result<String, String> {
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    Ok(node.my_id())
}

// ============ CONTACT BOOK COMMANDS ============

/// Add a new contact
#[tauri::command]
async fn add_contact(
    node_id: String,
    nickname: String,
    notes: Option<String>,
    state: State<'_, AppState>
) -> Result<Contact, String> {
    let contact = {
        let mut contacts = state.contacts.lock().unwrap();
        contacts.add(node_id, nickname, notes)
    };
    save_contacts(&state)?;
    Ok(contact)
}

/// Remove a contact
#[tauri::command]
async fn remove_contact(node_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let removed = {
        let mut contacts = state.contacts.lock().unwrap();
        contacts.remove(&node_id)
    };
    if removed {
        save_contacts(&state)?;
    }
    Ok(removed)
}

/// Update contact nickname
#[tauri::command]
async fn update_contact_nickname(
    node_id: String,
    nickname: String,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let updated = {
        let mut contacts = state.contacts.lock().unwrap();
        contacts.update_nickname(&node_id, nickname)
    };
    if updated {
        save_contacts(&state)?;
    }
    Ok(updated)
}

/// List all contacts
#[tauri::command]
async fn list_contacts(state: State<'_, AppState>) -> Result<Vec<Contact>, String> {
    let contacts = state.contacts.lock().unwrap();
    Ok(contacts.list().into_iter().cloned().collect())
}

/// Connect to a contact and check if they're online (ping with timeout)
#[tauri::command]
async fn ping_contact(node_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    match node.connect_to_contact(&node_id).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Helper to save contacts to disk
fn save_contacts(state: &State<'_, AppState>) -> Result<(), String> {
    let contacts = state.contacts.lock().unwrap();
    let path_guard = state.contacts_path.lock().unwrap();
    
    if let Some(path) = path_guard.as_ref() {
        contacts.save(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============ FILE SHARING COMMANDS ============

/// Share a file and return the ticket string
#[tauri::command]
async fn share_file(path: String, state: State<'_, AppState>) -> Result<String, String> {
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }
    
    let ticket = node.share_file(&file_path).await.map_err(|e| e.to_string())?;
    Ok(ticket)
}

/// Share bytes directly (for Android content:// URI support)
#[tauri::command]
async fn share_bytes(bytes: Vec<u8>, filename: String, state: State<'_, AppState>) -> Result<String, String> {
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    let ticket = node.share_bytes(&bytes, &filename).await.map_err(|e| e.to_string())?;
    Ok(ticket)
}

/// Download a file using ticket
#[tauri::command]
async fn download_file(ticket: String, state: State<'_, AppState>, app: tauri::AppHandle) -> Result<String, String> {
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    let save_dir = get_download_dir(&app);
    
    if !save_dir.exists() {
         std::fs::create_dir_all(&save_dir)
            .map_err(|e| format!("Failed to create download dir {:?}: {}", save_dir, e))?;
    }

    let filename = node.download_file(&ticket, &save_dir)
        .await
        .map_err(|e| format!("Download failed to {:?}: {}", save_dir, e))?;
    
    let full_path = save_dir.join(&filename);
    Ok(full_path.display().to_string())
}

/// Generate QR code as SVG string
#[tauri::command]
fn generate_qr_svg(data: String) -> Result<String, String> {
    let code = QrCode::new(data.as_bytes()).map_err(|e| e.to_string())?;
    let svg_str = code.render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#ffffff"))
        .light_color(svg::Color("#1a1a1a"))
        .build();
    Ok(svg_str)
}

/// Get download directory (platform specific)
fn get_download_dir(app: &tauri::AppHandle) -> PathBuf {
    #[cfg(target_os = "android")]
    {
        app.path().document_dir()
            .unwrap_or_else(|_| app.path().app_data_dir().unwrap_or(PathBuf::from("/data/local/tmp")))
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        if let Some(user_dirs) = UserDirs::new() {
            user_dirs.download_dir()
                .unwrap_or(&PathBuf::from("."))
                .to_path_buf()
        } else {
            PathBuf::from(".")
        }
    }
}
// ============ MARKETPLACE COMMANDS ============

/// Get random broadcast delay (1-4 seconds) for marketplace
#[tauri::command]
fn get_broadcast_delay() -> u64 {
    K2Marketplace::get_broadcast_delay()
}

/// Join a marketplace topic (Real P2P via iroh-gossip)
#[tauri::command]
async fn join_topic(topic: String, action: String, state: State<'_, AppState>) -> Result<String, String> {
    println!("[K2] 🔗 Joining topic: {} for action: {}", topic, action);
    
    // Get node from state
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized. Please wait for node to start.")?
    };
    
    // Convert topic string to TopicId
    let topic_id = K2Marketplace::topic_to_id(&topic);
    println!("[K2] Topic ID: {:?}", topic_id);
    
    // Actually subscribe to the topic via iroh-gossip with TRacker Discovery
    match node.subscribe_topic_with_discovery(topic_id).await {
        Ok(_topic_handle) => {
            println!("[K2] ✅ Successfully joined topic: {}", topic);
            Ok(format!("Successfully joined topic: {}", topic))
        }
        Err(e) => {
            println!("[K2] ❌ Failed to join topic: {}", e);
            Err(format!("Failed to join topic: {}", e))
        }
    }
}

/// Classify marketplace intent using Groq API (called from backend to avoid CORS)
#[tauri::command]
async fn classify_intent(user_prompt: String, api_key: String, base_url: Option<String>, model: Option<String>) -> Result<serde_json::Value, String> {
    println!("[K2] 🧠 Classifying intent: {}", user_prompt);
    
    let base_url = base_url.unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
    let model = model.unwrap_or_else(|| "llama-3.3-70b-versatile".to_string());
    
    let client = reqwest::Client::new();
    
    let system_prompt = r#"Bạn là AI phân tích yêu cầu mua bán trên K2 Marketplace. Phân tích ý định của người dùng và trích xuất thông tin.

Các topic:
- "Digital Assets": Video, Images, Audio, Token, License | Key | Secret, Document, Source Code, Dataset
- "Goods": Fashion, Electronics & Devices, Books & Learning, Sports & Travel  
- "Freelance Job": Tech & IT, Design & Creative, Writing & Translation, Marketing & Sales

Các action:
- "buy": Người dùng muốn MUA
- "sell": Người dùng muốn BÁN
- "exchange": Người dùng muốn TRAO ĐỔI

Trả về JSON theo format:
{
  "topic": "Digital Assets" | "Goods" | "Freelance Job",
  "selection": { "subtopic": "..." } hoặc { "category": "...", "skill": "..." },
  "action": "buy" | "sell" | "exchange",
  "description": "mô tả yêu cầu"
}"#;

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "response_format": {"type": "json_object"}
    });
    
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        println!("[K2] ❌ Groq API error: {} - {}", status, error_text);
        return Err(format!("Groq API error: {} - {}", status, error_text));
    }
    
    let data: serde_json::Value = response.json().await.map_err(|e| format!("JSON parse error: {}", e))?;
    
    // Extract content from response
    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("No content in response")?;
    
    let result: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("Content parse error: {}", e))?;
    
    println!("[K2] ✅ Classification result: {:?}", result);
    Ok(result)
}

/// Call K2 Endpoint (bypass SSL cert errors)
#[tauri::command]
async fn classify_k2_endpoint(user_prompt: String) -> Result<serde_json::Value, String> {
    println!("[K2] 🌍 Calling K2 Endpoint (ignoring SSL): {}", user_prompt);
    
    // Create client that ignores invalid certs
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("https://139.59.125.159/post?user_input={}", urlencoding::encode(&user_prompt));
    
    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("K2 Endpoint error: {}", response.status()));
    }
    
    let text = response.text().await.map_err(|e| e.to_string())?;
    println!("[K2] 📥 K2 Response: {}", text);
    
    // Parse JSON
    let result: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("JSON parse error: {}", e))?;
        
    Ok(result)
}

/// Message includes sender's nodeId so buyers can respond
#[tauri::command]
async fn broadcast_offer(topic: String, form_data: serde_json::Value, state: State<'_, AppState>) -> Result<String, String> {
    println!("[K2] 📡 Broadcasting offer to topic: {}", topic);
    
    // Get node from state
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    // Get sender's node ID
    let my_node_id = node.my_id();
    
    // Build message with nodeId included
    let mut payload = serde_json::Map::new();
    payload.insert("sender_node_id".to_string(), serde_json::Value::String(my_node_id.clone()));
    payload.insert("message_type".to_string(), serde_json::Value::String("offer".to_string()));
    payload.insert("topic".to_string(), serde_json::Value::String(topic.clone()));
    payload.insert("form_data".to_string(), form_data);
    payload.insert("timestamp".to_string(), serde_json::Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .into()
    ));
    
    let message = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    println!("[K2] 📦 Payload with nodeId: {:?}", my_node_id);
    
    // Convert topic and broadcast via iroh-gossip
    let topic_id = K2Marketplace::topic_to_id(&topic);
    match node.broadcast_message(topic_id, message).await {
        Ok(_) => {
            let offer_id = K2Marketplace::generate_id();
            println!("[K2] ✅ Offer broadcast successfully: {}", offer_id);
            Ok(format!("Offer broadcast: {}", offer_id))
        }
        Err(e) => {
            println!("[K2] ❌ Broadcast failed: {}", e);
            Err(format!("Broadcast failed: {}", e))
        }
    }
}

/// Send interest response to a seller (Buyer -> Seller)
/// Broadcasts on same topic with message_type="interest"
#[tauri::command]
async fn send_interest(topic: String, seller_node_id: String, form_data: serde_json::Value, state: State<'_, AppState>) -> Result<String, String> {
    println!("[K2] 💰 Sending interest to seller: {}", seller_node_id);
    
    // Get node from state
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    // Get buyer's node ID
    let my_node_id = node.my_id();
    
    // Build interest message
    let mut payload = serde_json::Map::new();
    payload.insert("sender_node_id".to_string(), serde_json::Value::String(my_node_id.clone()));
    payload.insert("target_node_id".to_string(), serde_json::Value::String(seller_node_id.clone()));
    payload.insert("message_type".to_string(), serde_json::Value::String("interest".to_string()));
    payload.insert("topic".to_string(), serde_json::Value::String(topic.clone()));
    payload.insert("form_data".to_string(), form_data);
    payload.insert("timestamp".to_string(), serde_json::Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .into()
    ));
    
    let message = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    println!("[K2] 📦 Interest from {} to {}", my_node_id, seller_node_id);
    
    // Broadcast interest on same topic
    let topic_id = K2Marketplace::topic_to_id(&topic);
    match node.broadcast_message(topic_id, message).await {
        Ok(_) => {
            println!("[K2] ✅ Interest sent successfully");
            Ok(format!("Interest sent to {}", seller_node_id))
        }
        Err(e) => {
            println!("[K2] ❌ Interest send failed: {}", e);
            Err(format!("Failed to send interest: {}", e))
        }
    }
}

/// Listen for offers on a topic (for Buyers)
/// Returns received offers as JSON array
#[tauri::command]
async fn listen_offers(topic: String, timeout_secs: u64, state: State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    use futures_util::StreamExt;
    use iroh_gossip::api::Event;
    
    println!("[K2] 👂 Listening for offers on topic: {} (timeout: {}s)", topic, timeout_secs);
    
    // Get node from state
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    // Subscribe to topic
    let topic_id = K2Marketplace::topic_to_id(&topic);
    let gossip_topic = node.subscribe_topic(topic_id).await.map_err(|e| e.to_string())?;
    
    // Split into sender and receiver (like example 12)
    let (_sender, mut receiver) = gossip_topic.split();
    
    let mut received_offers: Vec<serde_json::Value> = Vec::new();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();
    
    // Listen for messages until timeout (like example 12)
    while start.elapsed() < timeout {
        match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            receiver.next()
        ).await {
            Ok(Some(Ok(event))) => {
                match event {
                    Event::Received(msg) => {
                        // Try to parse as JSON
                        if let Ok(offer) = serde_json::from_slice::<serde_json::Value>(&msg.content) {
                            println!("[K2] 📨 Received offer: {:?}", offer);
                            received_offers.push(offer);
                        }
                    }
                    Event::NeighborUp(id) => {
                        println!("[K2] 🟢 Peer connected: {}...", &id.to_string()[..8]);
                    }
                    Event::NeighborDown(id) => {
                        println!("[K2] 🔴 Peer disconnected: {}...", &id.to_string()[..8]);
                    }
                    _ => {}
                }
            }
            Ok(Some(Err(e))) => {
                println!("[K2] ⚠️ Receiver error: {}", e);
            }
            Ok(None) => break, // Stream ended
            Err(_) => continue, // Timeout, try again
        }
    }
    
    println!("[K2] 📭 Listen complete. Received {} offers", received_offers.len());
    Ok(received_offers)
}

/// Start listening for offers in background (Real-time via Tauri Events)
/// Emits "k2://offer-received" event to frontend when offer is received
#[tauri::command]
async fn start_listening(topic: String, app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use futures_util::StreamExt;
    use iroh_gossip::api::Event;
    use tauri::Emitter;
    
    println!("[K2] 🎧 Starting real-time listener for topic: {}", topic);
    
    // Get node from state
    let node = {
        let guard = state.node.lock().unwrap();
        guard.clone().ok_or("Node not initialized")?
    };
    
    // Subscribe to topic with Discovery (Tracker)
    let topic_id = K2Marketplace::topic_to_id(&topic);
    println!("[K2] 🔍 Connecting to tracker and peers...");
    let gossip_topic = node.subscribe_topic_with_discovery(topic_id).await.map_err(|e| e.to_string())?;
    
    // Split into sender and receiver
    let (sender, mut receiver) = gossip_topic.split();
    
    // Spawn background task to listen and emit events
    let topic_clone = topic.clone();
    tokio::spawn(async move {
        println!("[K2] 🔊 Listener task started for topic: {}", topic_clone);
        
        // Keep sender alive - DO NOT DROP IT
        let _keep_alive = sender;
        
        loop {
            match receiver.next().await {
                Some(Ok(event)) => {
                    println!("[K2] 📬 Event received: {:?}", event);
                    match event {
                        Event::Received(msg) => {
                            println!("[K2] 📨 Message content length: {} bytes", msg.content.len());
                            // Try to parse as JSON
                            if let Ok(offer) = serde_json::from_slice::<serde_json::Value>(&msg.content) {
                                println!("[K2] 📨 Real-time offer received: {:?}", offer);
                                // Emit event to frontend
                                let _ = app_handle.emit("k2://offer-received", offer);
                            } else {
                                println!("[K2] ⚠️ Failed to parse message as JSON");
                            }
                        }
                        Event::NeighborUp(id) => {
                            println!("[K2] 🟢 Peer connected: {}", id.to_string());
                            let _ = app_handle.emit("k2://peer-connected", id.to_string());
                        }
                        Event::NeighborDown(id) => {
                            println!("[K2] 🔴 Peer disconnected: {}", id.to_string());
                            let _ = app_handle.emit("k2://peer-disconnected", id.to_string());
                        }
                        other => {
                            println!("[K2] 📭 Other event: {:?}", other);
                        }
                    }
                }
                Some(Err(e)) => {
                    println!("[K2] ⚠️ Listener error: {}", e);
                }
                None => {
                    println!("[K2] 🔇 Receiver stream closed for topic: {}", topic_clone);
                    break;
                }
            }
        }
        
        println!("[K2] 🔇 Listener task ended for topic: {}", topic_clone);
    });
    
    Ok(format!("Started listening on topic: {}", topic))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            #[cfg(mobile)]
            app.handle().plugin(tauri_plugin_barcode_scanner::init())?;
            #[cfg(target_os = "android")]
            app.handle().plugin(tauri_plugin_android_fs::init())?;
            Ok(())
        })
        .manage(AppState {
            node: Mutex::new(None),
            contacts: Mutex::new(ContactBook::default()),
            contacts_path: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            init_node,
            get_my_node_id,
            // Contact book commands
            add_contact,
            remove_contact,
            update_contact_nickname,
            list_contacts,
            ping_contact,
            // File sharing commands
            share_file,
            share_bytes,
            download_file,
            generate_qr_svg,
            // Marketplace commands
            classify_intent,
            classify_k2_endpoint,
            get_broadcast_delay,
            join_topic,
            broadcast_offer,
            send_interest,
            listen_offers,
            start_listening
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
