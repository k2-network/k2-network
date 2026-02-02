use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
use tauri::Manager;
use k2_core::{K2Node, Contact, ContactBook};
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
            generate_qr_svg
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
