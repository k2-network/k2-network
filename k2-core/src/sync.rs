//! K2 Sync Manager - Syncthing-style folder synchronization
//! 
//! Provides two-way synchronization between local file system and iroh-docs.
//! Uses `notify` for local file watching and `iroh-docs` subscriptions for remote updates.

use anyhow::{Context, Result};
use iroh_docs::{NamespaceId, AuthorId, Entry};
use iroh_docs::engine::LiveEvent;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use futures::StreamExt;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use serde::{Serialize, Deserialize};

use crate::{K2DocsClient, K2DocHandle, K2Blob};
use iroh::protocol::ProtocolHandler;
use iroh_docs::{api::protocol::ShareMode, DocTicket, Capability};
use filetime::FileTime;

pub const SYNC_INVITE_ALPN: &[u8] = b"k2/sync-invite/1";

/// Configuration for a synchronized folder (Stored in iroh-docs)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFolderConfig {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub sync_interval: u32,       // in minutes
    pub sync_mode: String,       // "proactive" | "passive"
    pub sync_enabled: bool,
    pub linked_devices: HashMap<String, String>, // NodeID -> Status (NotSent, Invited, Accepted, Rejected)
}

/// Combined folder configuration and live status (For UI/API)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFolderInfo {
    #[serde(flatten)]
    pub config: SyncFolderConfig,
    pub status: SyncStatus,
    pub is_pending: bool,
    pub remote_source: Option<String>,
}

/// Configuration for a peer device (Stored in iroh-docs)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeviceConfig {
    pub id: String,
    pub name: String,
    pub device_type: String,     // "Laptop" | "Desktop" | "Mobile"
    pub node_id: String,
}

/// Global settings for the sync machine (Stored in iroh-docs)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSettings {
    pub local_logo: Option<String>, // Base64 or path, default to k2Logo in UI if None
    pub display_name: Option<String>,
}

/// Information about a folder being synchronized
#[derive(Debug, Clone)]
pub struct SyncFolder {
    pub path: PathBuf,
    pub doc_id: NamespaceId,
    pub author_id: AuthorId,
    pub handle: K2DocHandle,
}

/// Sync status for a folder
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncStatus {
    Idle,           // No changes detected
    Pending,        // Changes detected but not synced (passive mode)
    Syncing,        // Currently syncing
    Error(String),  // Last sync failed
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncProtocolMessage {
    Invitation {
        folder_id: String,
        folder_name: String,
        ticket: String,
    },
    QueryStatus(String), // Folder ID
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncStatusResponse {
    Ack,
    Reject,
    Pending,
    Accepted,
    Left,
}

/// Metadata stored in iroh-docs for each file (besides the native hash/size)
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncFileMeta {
    pub mtime: u64, // Unix timestamp in seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDeviceInfo {
    pub config: SyncDeviceConfig,
    pub status: String, // "online" or "offline"
}

/// The Sync Manager coordinates local FS and remote P2P document state
#[derive(Clone, Debug)]
pub struct SyncManager {
    docs_client: K2DocsClient,
    blob_client: K2Blob,
    endpoint: iroh::Endpoint,
    active_folders: Arc<TokioMutex<HashMap<NamespaceId, SyncFolder>>>,
    settings_handle: Arc<TokioMutex<Option<K2DocHandle>>>,
    folder_status: Arc<TokioMutex<HashMap<String, SyncStatus>>>,
    device_status: Arc<TokioMutex<HashMap<String, bool>>>,
}

impl SyncManager {
    pub fn new(docs_client: K2DocsClient, blob_client: K2Blob, endpoint: iroh::Endpoint) -> Self {
        Self {
            docs_client,
            blob_client,
            endpoint,
            active_folders: Arc::new(TokioMutex::new(HashMap::new())),
            settings_handle: Arc::new(TokioMutex::new(None)),
            folder_status: Arc::new(TokioMutex::new(HashMap::new())),
            device_status: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    /// Initialize the Sync Manager and load existing configurations
    pub async fn init(&self) -> Result<()> {
        let mut found_id = None;
        let docs_list = self.docs_client.list_documents().await?;
        
        println!("[K2-Sync] 🔍 Scanning for existing sync settings doc (among {} docs)...", docs_list.len());
        
        for id in docs_list {
            if let Some(h) = self.docs_client.open_doc(id).await? {
                if let Ok(Some(marker)) = h.get(b"__k2_sync_settings_marker__").await {
                    if marker == b"k2-sync-v1" {
                        found_id = Some(id);
                        println!("[K2-Sync] ✅ Found persistent sync settings: {}", id);
                        break;
                    }
                }
            }
        }

        let handle = if let Some(id) = found_id {
            self.docs_client.open_doc(id).await?.unwrap()
        } else {
            println!("[K2-Sync] ✨ Creating new persistent sync settings doc...");
            let h = self.docs_client.create_doc().await?;
            h.put(b"__k2_sync_settings_marker__", b"k2-sync-v1").await?;
            h
        };

        {
            let mut guard = self.settings_handle.lock().await;
            *guard = Some(handle);
        }
        
        // Background check for all configured devices
        let devices = self.list_devices().await?;
        let manager = self.clone();
        tokio::spawn(async move {
            println!("[K2-Sync] 📡 Checking online status for {} devices in background...", devices.len());
            for dev in devices {
                manager.check_device_online(&dev.config.node_id).await;
            }
        });

        // Auto-start active folders from config
        self.start_configured_folders().await?;
        
        Ok(())
    }

    async fn handle(&self) -> Result<K2DocHandle> {
        let guard = self.settings_handle.lock().await;
        guard.as_ref()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("SyncManager not initialized"))
    }

    /// Start all folders that are enabled in the configuration
    async fn start_configured_folders(&self) -> Result<()> {
        let folders = self.list_folders().await?;
        if folders.is_empty() {
            println!("[K2-Sync] ℹ️ No configured folders to start.");
            return Ok(());
        }
        
        println!("[K2-Sync] 📂 Starting {} configured folders...", folders.len());
        for config in folders {
            // [Temporary] Ignore pause sync: if config.sync_enabled && config.path.exists() {
            if config.path.exists() {
                if let Ok(doc_id) = config.id.parse::<NamespaceId>() {
                    let _ = self.register_folder(config.path, doc_id).await;
                }
            }
        }
        Ok(())
    }

    // --- CONFIGURATION MANAGEMENT ---

    pub async fn list_folders(&self) -> Result<Vec<SyncFolderConfig>> {
        let results = self.handle().await?.list_prefix(b"folder:").await?;
        let mut folders = Vec::new();
        for (_key, value) in results {
            if let Ok(config) = serde_json::from_slice::<SyncFolderConfig>(&value) {
                folders.push(config);
            }
        }
        Ok(folders)
    }

    /// Get all folders with their live status, including pending ones from network
    pub async fn get_all_folders_info(&self) -> Result<Vec<SyncFolderInfo>> {
        let configs = self.list_folders().await?;
        let status_map = self.folder_status.lock().await;
        let mut infos = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // 1. Add local (active) folders
        for config in configs {
            seen_ids.insert(config.id.clone());
            let status = status_map.get(&config.id).cloned().unwrap_or(SyncStatus::Idle);
            infos.push(SyncFolderInfo {
                config,
                status,
                is_pending: false,
                remote_source: None,
            });
        }

        // 2. Scan network for pending folders
        if let Ok(handle) = self.handle().await {
            let pending_invites = handle.list_prefix(b"pending_invite:").await?;
            for (key, _value) in pending_invites {
                let folder_id = String::from_utf8_lossy(&key[15..]).to_string(); // Skip "pending_invite:"
                if !seen_ids.contains(&folder_id) {
                    // Create a dummy config for UI display
                    let config = SyncFolderConfig {
                        id: folder_id.clone(),
                        name: format!("Pending Folder ({})", &folder_id[..6]),
                        path: PathBuf::new(),
                        sync_interval: 60,
                        sync_mode: "proactive".to_string(),
                        sync_enabled: false,
                        linked_devices: HashMap::new(),
                    };
                    
                    infos.push(SyncFolderInfo {
                        config,
                        status: SyncStatus::Idle,
                        is_pending: true,
                        remote_source: Some("Network".to_string()),
                    });
                }
            }
        }

        Ok(infos)
    }

    /// Accept a folder from network by setting a local path
    pub async fn accept_folder_config(&self, folder_id: &str, local_path: PathBuf) -> Result<()> {
        let h = self.handle().await?;
        let invite_key = format!("pending_invite:{}", folder_id);
        
        let ticket_bytes = h.get(invite_key.as_bytes()).await?
            .ok_or_else(|| anyhow::anyhow!("Invitation not found for folder {}", folder_id))?;
            
        let ticket_str = String::from_utf8(ticket_bytes.to_vec())?;
        let ticket = ticket_str.parse::<DocTicket>()?;
        let doc_id = match &ticket.capability {
            Capability::Read(id) => *id,
            Capability::Write(id) => id.clone().into(),
        };

        // 1. Join the document
        println!("[K2-Sync] 🔗 Joining shared document {}...", doc_id);
        self.docs_client.import_doc(ticket.clone()).await?; // Need to clone or use reference if modifying
        
        // Extract the inviter nodes from the ticket
        let mut linked_devices = HashMap::new();
        for node_addr in &ticket.nodes {
            linked_devices.insert(node_addr.id.to_string(), "Accepted".to_string());
        }
        
        // 2. Create the configuration
        let config = SyncFolderConfig {
            id: folder_id.to_string(),
            name: format!("Shared Folder ({})", &folder_id[..6]), // Default name
            path: local_path,
            sync_interval: 60,
            sync_mode: "proactive".to_string(),
            sync_enabled: true,
            linked_devices,
        };
        
        // 3. Save locally and start sync
        self.add_folder_config(config).await?;
        
        // 4. Cleanup pending invitation
        h.delete(invite_key.as_bytes()).await?;
        
        println!("[K2-Sync] ✅ Successfully accepted folder {}", folder_id);
        Ok(())
    }

    /// Get the status of a specific folder
    pub async fn get_folder_status(&self, folder_id: &str) -> SyncStatus {
        let status_map = self.folder_status.lock().await;
        status_map.get(folder_id).cloned().unwrap_or(SyncStatus::Idle)
    }

    pub async fn add_folder_config(&self, mut config: SyncFolderConfig) -> Result<String> {
        // 1. Ensure we have a valid NamespaceId
        let doc_id = match config.id.parse::<NamespaceId>() {
            Ok(id) => id,
            Err(_) => {
                // Invalid ID from frontend (like Date.now()), create a new doc
                println!("[K2-Sync] 🆕 Creating new document for folder: {}", config.name);
                let h = self.docs_client.create_doc().await?;
                let new_id = h.id();
                config.id = new_id.to_string();
                new_id
            }
        };

        // 2. Register active sync only if NOT already registered
        let is_already_active = {
            let active = self.active_folders.lock().await;
            active.contains_key(&doc_id)
        };

        if config.path.exists() && !is_already_active {
            let manager_cloned = self.clone();
            let path_cloned = config.path.clone();
            tokio::spawn(async move {
                let _ = manager_cloned.register_folder(path_cloned.into(), doc_id).await;
            });
        }

        // 3. Save config to settings doc
        let key = format!("folder:{}", config.id);
        self.handle().await?.put_json(key.as_bytes(), &config).await?;

        // 4. Check for new devices and trigger invitations in background
        let manager = self.clone();
        let config_id = config.id.clone();
        let devices = config.linked_devices.clone();
        
        tokio::spawn(async move {
            // Small delay to let the node announce itself and finish initial scan overhead
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            
            for (node_id, status) in devices {
                if node_id == "undefined" || node_id.is_empty() {
                    continue;
                }
                if status == "NotSent" {
                    println!("[K2-Sync] 📨 Attempting to invite device: {}", node_id);
                    match manager.send_invitation_direct(&node_id, &config_id).await {
                        Ok(_) => {
                            // Update status in settings doc
                            if let Ok(mut current_config) = manager.get_folder_config_by_id(&config_id).await {
                                if let Some(s) = current_config.linked_devices.get_mut(&node_id) {
                                    *s = "Invited".to_string();
                                    let key = format!("folder:{}", config_id);
                                    let _ = manager.handle().await.unwrap().put_json(key.as_bytes(), &current_config).await;
                                }
                            }
                        }
                        Err(e) => {
                            println!("[K2-Sync] 📴 Failed to invite device {}: {:?}", node_id, e);
                        }
                    }
                }
            }
        });

        Ok(config.id)
    }

    /// Join a remote settings document and update local state (Side B)
    pub async fn join_settings_doc(&self, ticket: DocTicket) -> Result<()> {
        println!("[K2-Sync] 📩 Joining settings document: {}", ticket);
        let h = self.docs_client.import_doc(ticket).await?;
        
        // Ensure marker
        h.put(b"__k2_sync_settings_marker__", b"k2-sync-v1").await?;
        
        {
            let mut guard = self.settings_handle.lock().await;
            *guard = Some(h);
        }
        
        // Refresh configurations
        println!("[K2-Sync] 📂 Refreshing configurations from new settings doc...");
        self.start_configured_folders().await?;
        Ok(())
    }

    pub async fn remove_folder_config(&self, id: &str) -> Result<()> {
        let key = format!("folder:{}", id);
        let h = self.handle().await?;
        
        // 1. Mark as "Left" (Soft delete tombstone) for peers to know we've left
        let left_key = format!("left_folder:{}", id);
        h.put(left_key.as_bytes(), b"1").await?;
        
        // 2. Delete the actual config
        h.delete(key.as_bytes()).await?;
        
        // 3. Also stop active sync if running
        if let Ok(doc_id) = id.parse::<NamespaceId>() {
            let mut active = self.active_folders.lock().await;
            active.remove(&doc_id);
        }
        Ok(())
    }

    pub async fn list_devices(&self) -> Result<Vec<SyncDeviceInfo>> {
        let results = self.handle().await?.list_prefix(b"device:").await?;
        let mut devices = Vec::new();
        let status_map = self.device_status.lock().await;
        
        for (_key, value) in results {
            if let Ok(config) = serde_json::from_slice::<SyncDeviceConfig>(&value) {
                let is_online = status_map.get(&config.node_id).copied().unwrap_or(false);
                devices.push(SyncDeviceInfo {
                    config,
                    status: if is_online { "online".to_string() } else { "offline".to_string() },
                });
            }
        }
        Ok(devices)
    }

    pub async fn add_device_config(&self, config: SyncDeviceConfig) -> Result<()> {

        let key = format!("device:{}", config.id);
        self.handle().await?.put_json(key.as_bytes(), &config).await?;
        Ok(())
    }

    pub async fn remove_device_config(&self, id: &str) -> Result<()> {
        let key = format!("device:{}", id);
        self.handle().await?.delete(key.as_bytes()).await?;
        Ok(())
    }

    pub async fn get_settings(&self) -> Result<SyncSettings> {
        let settings: Option<SyncSettings> = self.handle().await?.get_json(b"settings").await?;
        Ok(settings.unwrap_or(SyncSettings { local_logo: None, display_name: None }))
    }

    pub async fn update_settings(&self, settings: SyncSettings) -> Result<()> {
        self.handle().await?.put_json(b"settings", &settings).await
    }

    /// Trigger a manual sync for a specific folder
    pub async fn sync_now(&self, folder_id: &str) -> Result<()> {
        let doc_id: NamespaceId = folder_id.parse().context("Invalid folder ID")?;
        
        // 1. Force a local scan first to make sure we have the latest
        self.sync_local_to_remote(doc_id).await?;
        
        // 2. Perform the actual network sync (force=true)
        self.perform_sync(doc_id, true).await
    }

    /// Internal method to perform the actual network reconciliation with linked peers
    async fn perform_sync(&self, doc_id: NamespaceId, force: bool) -> Result<()> {
        let mut config = match self.get_folder_config_by_doc(doc_id).await {
            Some(c) => c,
            None => return Err(anyhow::anyhow!("Folder config not found")),
        };

        if !config.sync_enabled && !force {
            return Ok(());
        }

        // Get linked devices and resolve their addresses
        let mut peer_addrs = Vec::new();
        let mut config_changed = false;
        let mut devices_to_remove = Vec::new();

        for (node_id_str, status) in config.linked_devices.iter_mut() {
            if node_id_str == "undefined" || node_id_str.is_empty() {
                continue;
            }
            if status == "Accepted" {
                if let Ok(node_id) = node_id_str.parse::<iroh_base::PublicKey>() {
                    peer_addrs.push(iroh_base::EndpointAddr::from(node_id));
                }
            } else if status == "NotSent" {
                println!("[K2-Sync] 📨 Device {} is 'NotSent', attempting invitation...", node_id_str);
                match self.send_invitation_direct(node_id_str, &config.id).await {
                    Ok(_) => {
                        *status = "Invited".to_string();
                        config_changed = true;
                    }
                    Err(e) => {
                        println!("[K2-Sync] 📴 Failed to invite device {}: {:?}", node_id_str, e);
                    }
                }
            } else if status == "Invited" {
                println!("[K2-Sync] 🔍 Polling device {} for folder {}...", node_id_str, config.name);
                
                match self.query_peer_folder_status(node_id_str, &config.id).await {
                    Ok(SyncStatusResponse::Accepted) => {
                        println!("[K2-Sync] 🎉 Device {} has ACCEPTED folder {}", node_id_str, config.name);
                        *status = "Accepted".to_string();
                        config_changed = true;
                        if let Ok(node_id) = node_id_str.parse::<iroh_base::PublicKey>() {
                            peer_addrs.push(iroh_base::EndpointAddr::from(node_id));
                        }
                    }
                    Ok(SyncStatusResponse::Pending) => {
                        println!("[K2-Sync] ⏳ Device {} is still PENDING for folder {}", node_id_str, config.name);
                    }
                    Ok(SyncStatusResponse::Reject) | Ok(SyncStatusResponse::Left) => {
                        println!("[K2-Sync] ❌ Device {} has REJECTED or LEFT folder {}. Removing...", node_id_str, config.name);
                        devices_to_remove.push(node_id_str.clone());
                        config_changed = true;
                    }
                    Ok(SyncStatusResponse::Ack) => {
                        // Just an ACK, do nothing
                    }
                    Err(_) => {
                        // Peer offline, do nothing, keep "Invited" status
                    }
                }
            }
        }

        // Remove rejected devices
        for id in devices_to_remove {
            config.linked_devices.remove(&id);
        }

        // Save updated config if any status changed
        if config_changed {
            let key = format!("folder:{}", config.id);
            self.handle().await?.put_json(key.as_bytes(), &config).await?;
        }

        if peer_addrs.is_empty() {
            println!("[K2-Sync] ℹ️ No active peers for '{}'", config.name);
            return Ok(());
        }

        let handle = self.docs_client.open_doc(doc_id).await?.context("Doc not found")?;
        
        println!("[K2-Sync] 🔄 Syncing '{}' with {} active peers", config.name, peer_addrs.len());
        handle.sync(peer_addrs).await?;

        // After syncing state with peers, explicitly reconcile the remote state to the local disk
        let _ = self.reconcile_remote_to_local(doc_id).await;

        Ok(())
    }

    /// Iterates through the document and ensures all remote files exist on the local disk
    pub async fn reconcile_remote_to_local(&self, doc_id: NamespaceId) -> Result<()> {
        let (path, handle) = self.get_folder_path_and_handle(doc_id).await?;

        let entries = handle.list_all().await?;
        println!("[K2-Sync] 📥 Reconciling remote changes to local disk for {:?} ({} entries found in doc)", path, entries.len());

        let mut reconciled_count = 0;
        
        for entry in entries {
            let rel_path = String::from_utf8(entry.key().to_vec()).unwrap_or_default();
            if rel_path.is_empty() || rel_path.starts_with("meta:") { continue; }
            
            let file_path = path.join(&rel_path);
            let hash = entry.content_hash();
            
            // 1. Check if local file exists and matches hash
            if file_path.exists() {
                if let Ok((local_hash, _)) = self.blob_client.add_file(file_path.clone()).await {
                    if local_hash == hash {
                        // Content matches! Just restore mtime if needed
                        let meta_key = format!("meta:{}", rel_path);
                        match handle.get_json::<SyncFileMeta>(meta_key.as_bytes()).await {
                            Ok(Some(meta)) => {
                                let ft = FileTime::from_unix_time(meta.mtime as i64, 0);
                                if let Ok(local_meta) = std::fs::metadata(&file_path) {
                                    let local_mtime = FileTime::from_last_modification_time(&local_meta);
                                    if local_mtime.unix_seconds() != ft.unix_seconds() {
                                        let _ = filetime::set_file_mtime(&file_path, ft);
                                        println!("[K2-Sync] 📅 Restored mtime for existing {}: {} -> {}", rel_path, local_mtime.unix_seconds(), ft.unix_seconds());
                                    }
                                }
                            },
                            Ok(None) => {
                                // Meta not arrived yet, we'll catch it in the listener
                            },
                            Err(e) => eprintln!("[K2-Sync] ❌ Error getting meta for {}: {:?}", rel_path, e),
                        }
                        reconciled_count += 1;
                        continue;
                    }
                }
            }

            // 2. Efficiently export from blob store to disk
            match self.blob_client.export(hash, file_path.clone()).await {
                Ok(_) => {
                    reconciled_count += 1;
                    
                    // 3. Restore mtime from metadata
                    let meta_key = format!("meta:{}", rel_path);
                    if let Ok(Some(meta)) = handle.get_json::<SyncFileMeta>(meta_key.as_bytes()).await {
                        let ft = FileTime::from_unix_time(meta.mtime as i64, 0);
                        let _ = filetime::set_file_mtime(&file_path, ft);
                        println!("[K2-Sync] 📅 Set mtime for new file {}: {}", rel_path, meta.mtime);
                    }
                }
                Err(_) => {}
            }
        }
        
        println!("[K2-Sync] ✅ Reconciled {} files to disk using streaming export.", reconciled_count);
        Ok(())
    }

    // --- ACTIVE SYNC LOGIC ---
    pub async fn register_folder(&self, path: PathBuf, doc_id: NamespaceId) -> Result<()> {
        let author_id = self.docs_client.default_author().await?;
        let handle = self.docs_client.open_doc(doc_id).await?
            .ok_or_else(|| anyhow::anyhow!("Failed to open doc for sync folder"))?;
            
        let abs_path = if path.is_absolute() {
            path
        } else {
            std::fs::canonicalize(&path).context("Failed to get absolute path")?
        };
        
        let folder = SyncFolder {
            path: abs_path.clone(),
            doc_id,
            author_id,
            handle,
        };

        {
            let mut folders = self.active_folders.lock().await;
            folders.insert(doc_id, folder);
        }

        println!("[K2-Sync] 🛰️ Registered sync folder: {:?} -> {}", abs_path, doc_id);

        // 1. Initial scan (first-time hash calculation - push local to remote)
        self.sync_local_to_remote(doc_id).await?;
        
        // 1.5. Initial reconcile (pull remote to local disk)
        let _ = self.reconcile_remote_to_local(doc_id).await;
        
        // 2. Start local file watcher (real-time detection)
        self.spawn_watcher(doc_id).await?;
        
        // 3. Start remote event listener (download from peers)
        self.spawn_remote_listener(doc_id).await?;
        
        // 4. Start interval-based sync loop
        self.spawn_sync_loop(doc_id).await?;

        Ok(())
    }

    /// Spawn a periodic sync loop based on folder's syncInterval
    async fn spawn_sync_loop(&self, doc_id: NamespaceId) -> Result<()> {
        let manager = self.clone();
        
        tokio::spawn(async move {
            // Wait for the first interval before starting (initial scan already done)
            loop {
                // Read current folder config to get interval and mode
                let config = match manager.get_folder_config_by_doc(doc_id).await {
                    Some(c) => c,
                    None => {
                        println!("[K2-Sync] ⏹️ Folder removed, stopping sync loop for {}", doc_id);
                        break;
                    }
                };

                // Sleep for the configured interval
                let interval_secs = (config.sync_interval as u64) * 60;
                println!("[K2-Sync] ⏰ Next scan for '{}' in {} min", config.name, config.sync_interval);
                tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

                // [Temporary] Ignore pause sync
                // if !config.sync_enabled {
                //     continue;
                // }

                // Re-scan and detect changes
                println!("[K2-Sync] 🔄 Periodic scan of '{}'...", config.name);
                if let Err(e) = manager.sync_local_to_remote(doc_id).await {
                    eprintln!("[K2-Sync] ❌ Scan error for '{}': {:?}", config.name, e);
                    let mut status = manager.folder_status.lock().await;
                    status.insert(config.id.clone(), SyncStatus::Error(e.to_string()));
                    continue;
                }

                // Check linked devices
                if config.linked_devices.is_empty() {
                    let mut status = manager.folder_status.lock().await;
                    status.insert(config.id.clone(), SyncStatus::Idle);
                    continue;
                }

                // Check which devices are online (10s timeout each)
                let devices = match manager.list_devices().await {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let mut online_count = 0u32;
                for (node_id_str, _) in &config.linked_devices {
                    if let Some(dev) = devices.iter().find(|d| d.config.node_id == *node_id_str) {
                        if manager.check_device_online(&dev.config.node_id).await {
                            online_count += 1;
                        }
                    }
                }

                if online_count == 0 {
                    println!("[K2-Sync] 📴 No linked devices online for '{}'", config.name);
                    let mut status = manager.folder_status.lock().await;
                    status.insert(config.id.clone(), SyncStatus::Idle);
                    continue;
                }

                // Proactive vs Passive decision
                if config.sync_mode == "proactive" {
                    println!("[K2-Sync] 🚀 Proactive mode: Initiating sync for '{}'", config.name);
                    let mut status = manager.folder_status.lock().await;
                    status.insert(config.id.clone(), SyncStatus::Syncing);
                    
                    if let Err(e) = manager.perform_sync(doc_id, false).await {
                        eprintln!("[K2-Sync] ❌ Sync error for '{}': {:?}", config.name, e);
                        *status.get_mut(&config.id).unwrap() = SyncStatus::Error(e.to_string());
                    }
                } else {
                    println!("[K2-Sync] ⏸️ Passive mode: Changes detected, waiting for manual sync for '{}'", config.name);
                    let mut status = manager.folder_status.lock().await;
                    status.insert(config.id.clone(), SyncStatus::Pending);
                }
            }
        });

        Ok(())
    }

    /// Check if a device is online by trying to connect (10s timeout)
    pub async fn check_device_online(&self, node_id_str: &str) -> bool {
        let node_id: iroh_base::PublicKey = if node_id_str.len() == 64 {
            match hex::decode(node_id_str) {
                Ok(bytes) => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    match iroh_base::PublicKey::from_bytes(&arr) {
                        Ok(id) => id,
                        Err(_) => return false,
                    }
                }
                Err(_) => return false,
            }
        } else {
            match node_id_str.parse() {
                Ok(id) => id,
                Err(_) => return false,
            }
        };

        // Attempt a ping-like connection with timeout
        let test = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.endpoint.connect(iroh_base::EndpointAddr::from(node_id), SYNC_INVITE_ALPN)
        ).await;

        let is_online = test.is_ok() && test.unwrap().is_ok();
        
        // Cache the status for the UI
        let mut cache = self.device_status.lock().await;
        cache.insert(node_id_str.to_string(), is_online);
        
        if is_online {
            println!("[K2-Sync] ✅ Device online: {}...", &node_id_str[..10]);
        } else {
            println!("[K2-Sync] 📴 Device {} is offline, will retry later", node_id_str);
        }

        is_online
    }

    /// Send a sync invitation directly to a target node via P2P connection
    pub async fn send_invitation_direct(&self, target_node_id: &str, folder_id: &str) -> Result<()> {
        let target_public_key = if target_node_id.len() == 64 {
            let bytes = hex::decode(target_node_id)?;
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            iroh_base::PublicKey::from_bytes(&arr)?
        } else {
            target_node_id.parse()?
        };

        let doc_id: NamespaceId = folder_id.parse().map_err(|_| anyhow::anyhow!("Invalid folder ID"))?;
        let config = self.get_folder_config_by_doc(doc_id).await
            .ok_or_else(|| anyhow::anyhow!("Folder config not found"))?;
            
        // Get the actual document handle for this folder
        let folder_handle = {
            let folders = self.active_folders.lock().await;
            if let Some(folder) = folders.get(&doc_id) {
                folder.handle.clone()
            } else {
                // Folder is not actively syncing, open it manually just to get the ticket
                self.docs_client.open_doc(doc_id).await?
                    .ok_or_else(|| anyhow::anyhow!("Document not found in database"))?
            }
        };
        
        let ticket: DocTicket = folder_handle.share(ShareMode::Write).await?;
        let ticket_str = ticket.to_string();

        println!("[K2-Sync] 📞 Calling {} to send invitation for folder {} (Bi-di Handshake)...", target_node_id, config.name);
        
        let connection = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.endpoint.connect(iroh_base::EndpointAddr::from(target_public_key), SYNC_INVITE_ALPN)
        ).await.map_err(|_| anyhow::anyhow!("Connection timeout after 30s"))??;
        
        let (mut send_stream, mut recv_stream) = connection.open_bi().await?;
        
        // Send Invitation Message
        let msg = SyncProtocolMessage::Invitation {
            folder_id: config.id.clone(),
            folder_name: config.name.clone(),
            ticket: ticket_str,
        };
        let msg_bytes = serde_json::to_vec(&msg)?;
        send_stream.write_all(&msg_bytes).await?;
        send_stream.finish()?; 
        
        // Wait for ACK
        println!("[K2-Sync] ⏳ Ticket sent, waiting for ACK from {}...", target_node_id);
        let mut ack = [0u8; 1];
        recv_stream.read_exact(&mut ack).await?;
        
        if ack[0] == 1 {
            println!("[K2-Sync] ✅ Invitation accepted/received by {}", target_node_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invitation rejected by remote peer"))
        }
    }

    /// Query a peer for their status on a specific folder
    pub async fn query_peer_folder_status(&self, target_node_id: &str, folder_id: &str) -> Result<SyncStatusResponse> {
        let target_public_key = target_node_id.parse::<iroh_base::PublicKey>()
            .map_err(|e| anyhow::anyhow!("Invalid node ID: {:?}", e))?;
            
        let connection = self.endpoint.connect(iroh_base::EndpointAddr::from(target_public_key), SYNC_INVITE_ALPN).await?;
        let (mut send_stream, mut recv_stream) = connection.open_bi().await?;
        
        let msg = SyncProtocolMessage::QueryStatus(folder_id.to_string());
        let msg_bytes = serde_json::to_vec(&msg)?;
        
        send_stream.write_all(&msg_bytes).await?;
        send_stream.finish()?;
        
        let mut resp_bytes = [0u8; 1];
        recv_stream.read_exact(&mut resp_bytes).await?;
        
        match resp_bytes[0] {
            1 => Ok(SyncStatusResponse::Reject),
            2 => Ok(SyncStatusResponse::Pending),
            3 => Ok(SyncStatusResponse::Accepted),
            4 => Ok(SyncStatusResponse::Left),
            _ => Ok(SyncStatusResponse::Reject), 
        }
    }

    /// Get folder config by its ID
    pub async fn get_folder_config_by_id(&self, id: &str) -> Result<SyncFolderConfig> {
        let key = format!("folder:{}", id);
        let config: SyncFolderConfig = self.handle().await?.get_json(key.as_bytes()).await?
            .ok_or_else(|| anyhow::anyhow!("Folder config not found"))?;
        Ok(config)
    }

    /// Get folder config that matches a doc_id
    async fn get_folder_config_by_doc(&self, doc_id: NamespaceId) -> Option<SyncFolderConfig> {
        let doc_id_str = doc_id.to_string();
        if let Ok(folders) = self.list_folders().await {
            folders.into_iter().find(|f| f.id == doc_id_str)
        } else {
            None
        }
    }

    /// Helper to get path and handle, falling back to db if folder is paused (not in active_folders)
    async fn get_folder_path_and_handle(&self, doc_id: NamespaceId) -> Result<(PathBuf, K2DocHandle)> {
        // Try active folders first
        {
            let folders = self.active_folders.lock().await;
            if let Some(folder) = folders.get(&doc_id) {
                return Ok((folder.path.clone(), folder.handle.clone()));
            }
        }
        
        // Fallback for paused folders
        let config = self.get_folder_config_by_doc(doc_id).await
            .ok_or_else(|| anyhow::anyhow!("Folder config not found"))?;
            
        let handle = self.docs_client.open_doc(doc_id).await?
            .ok_or_else(|| anyhow::anyhow!("Doc not found in storage"))?;
            
        Ok((config.path, handle))
    }


    /// Scans the local folder and pushes everything to the remote doc
    pub async fn sync_local_to_remote(&self, doc_id: NamespaceId) -> Result<()> {
        let (path, handle) = self.get_folder_path_and_handle(doc_id).await?;

        println!("[K2-Sync] 🔍 Initial scan of {:?}", path);
        let count = self.scan_dir_recursive(&path, &path, &handle).await?;
        println!("[K2-Sync] ⬆️ Pushed {} local files to remote document.", count);
        Ok(())
    }

    async fn scan_dir_recursive(&self, root: &Path, current: &Path, handle: &K2DocHandle) -> Result<usize> {
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                // Recursion
                count += Box::pin(self.scan_dir_recursive(root, &path, handle)).await?;
            } else {
                // Ignore .k2 files and system files
                let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                if path.extension().unwrap_or_default() == "k2" || 
                   file_name == "desktop.ini" || 
                   file_name == "thumbs.db" || 
                   file_name == ".ds_store" { 
                    continue; 
                }
                
                let rel_path = path.strip_prefix(root)?.to_string_lossy().to_string();
                println!("[K2-Sync] 📄 Found file: {}", rel_path);
                
                // 1. Add file to blob store using Reference mode (zero-copy, streaming hash)
                match self.blob_client.add_file(path.clone()).await {
                    Ok((hash, size)) => {
                        // 2. Only store the HASH in the document (ledger)
                        println!("[K2-Sync] 📝 Recording in Document: {} -> {}", rel_path, hash);
                        handle.set_hash(rel_path.as_bytes(), hash, size).await?;
                        
                        // 3. Store additional metadata (mtime)
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if let Ok(mtime) = metadata.modified() {
                                let unix_time = mtime.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                                let meta = SyncFileMeta { mtime: unix_time };
                                let meta_key = format!("meta:{}", rel_path);
                                let _ = handle.put_json(meta_key.as_bytes(), &meta).await;
                            }
                        }

                        count += 1;
                    }
                    Err(e) => {
                        eprintln!("[K2-Sync] ❌ Error indexing {}: {:?}", rel_path, e);
                    }
                }
            }
        }
        Ok(count)
    }

    /// Spawn a task to watch for local file changes
    async fn spawn_watcher(&self, doc_id: NamespaceId) -> Result<()> {
        let (path, handle): (PathBuf, K2DocHandle) = {
            let folders = self.active_folders.lock().await;
            let folder = folders.get(&doc_id).context("Folder not found")?;
            (folder.path.clone(), self.docs_client.open_doc(doc_id).await?.context("Doc not found")?)
        };

        let root: PathBuf = path.clone();
        
        let manager = self.clone(); // Clone OUTSIDE the spawn block
        tokio::spawn(async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel(100);
            
            let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            }).expect("Failed to create watcher");

            watcher.watch(&root, RecursiveMode::Recursive).expect("Failed to start watch");
            println!("[K2-Sync] 👀 Watching for local changes in {:?}", root);

            while let Some(event) = rx.recv().await {
                // Check if folder is still active
                {
                    let folders = manager.active_folders.lock().await;
                    if !folders.contains_key(&doc_id) {
                        println!("[K2-Sync] ⏹️ Folder removed, stopping watcher for {}", doc_id);
                        break;
                    }
                }

                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            for p in event.paths {
                                if !p.is_file() { continue; }
                                
                                // Ignore system files
                                let file_name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                                if file_name == "desktop.ini" || file_name == "thumbs.db" { continue; }

                                let rel_result = p.strip_prefix(&root);
                                if let Ok(rel_path) = rel_result {
                                    let key = rel_path.to_string_lossy().to_string();
                                    
                                    // DEBOUNCE: Use a small sleep to let multiple rapid events for same file settle
                                    // and then only spawn if we aren't already indexing this file
                                    let manager_cloned = manager.clone();
                                    let key_cloned = key.clone();
                                    let p_cloned = p.clone();
                                    let handle_cloned = handle.clone();
                                    
                                    tokio::spawn(async move {
                                        // Wait a bit for the "storm" of events to pass
                                        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                                        
                                        println!("[K2-Sync] 🔔 Change detected: {}", key_cloned);
                                        
                                        // 1. Add modified file to blob store (Streaming + Reference)
                                        // Note: Iroh handles deduplication internally, but we avoid redundant doc writes
                                        match manager_cloned.blob_client.add_file(p_cloned.clone()).await {
                                            Ok((hash, _)) => {
                                                // 2. Update hash in document
                                                if let Ok(metadata) = std::fs::metadata(&p_cloned) {
                                                    let size = metadata.len();
                                                    let mtime = metadata.modified()
                                                        .ok()
                                                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                                        .map(|d| d.as_secs())
                                                        .unwrap_or(0);

                                                    if let Err(e) = handle_cloned.set_hash(key_cloned.as_bytes(), hash, size).await {
                                                        eprintln!("[K2-Sync] ❌ Error updating hash for {}: {:?}", key_cloned, e);
                                                    } else {
                                                        // 3. Update mtime metadata
                                                        let meta = SyncFileMeta { mtime };
                                                        let meta_key = format!("meta:{}", key_cloned);
                                                        let _ = handle_cloned.put_json(meta_key.as_bytes(), &meta).await;
                                                        
                                                        println!("[K2-Sync] ⬆️ Sync UP (Hash+Meta): {}", key_cloned);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("[K2-Sync] ❌ Error re-indexing {}: {:?}", key_cloned, e);
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        // Handle deletions using Tombstone pattern
                        EventKind::Remove(_) => {
                            for p in event.paths {
                                // Ignore system files
                                let file_name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                                if file_name == "desktop.ini" || file_name == "thumbs.db" { continue; }

                                let rel_result = p.strip_prefix(&root);
                                if let Ok(rel_path) = rel_result {
                                    let key = rel_path.to_string_lossy().to_string();
                                    
                                    // For deletions, we want to be sure it's actually gone
                                    if !p.exists() {
                                        // We don't delete the key, we write a "tombstone" value
                                        if let Err(e) = handle.put(key.as_bytes(), b"__K2_SYNC_TOMBSTONE_DELETED__").await {
                                            eprintln!("[K2-Sync] ❌ Error marking tombstone for {}: {:?}", key, e);
                                        } else {
                                            println!("[K2-Sync] 🗑️ Local Delete (Tombstone UP): {}", key);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
            }
        });

        Ok(())
    }

    /// Spawn a task to listen for remote document updates
    async fn spawn_remote_listener(&self, doc_id: NamespaceId) -> Result<()> {
        let (path, handle): (PathBuf, K2DocHandle) = {
            let folders = self.active_folders.lock().await;
            let folder = folders.get(&doc_id).context("Folder not found")?;
            (folder.path.clone(), self.docs_client.open_doc(doc_id).await?.context("Doc not found")?)
        };

        let root: PathBuf = path.clone();
        let events = handle.subscribe().await?;
        let mut events: std::pin::Pin<Box<dyn futures::Stream<Item = Result<LiveEvent>> + Send>> = Box::pin(events);
        println!("[K2-Sync] 👂 Listening for remote updates on {}", doc_id);
        
        let manager = self.clone();
        tokio::spawn(async move {
            while let Some(event_res) = events.next().await {
                // Check if folder is still active
                {
                    let folders = manager.active_folders.lock().await;
                    if !folders.contains_key(&doc_id) {
                        println!("[K2-Sync] ⏹️ Folder removed, stopping remote listener for {}", doc_id);
                        break;
                    }
                }

                if let Ok(event) = event_res {
                    match event {
                        LiveEvent::InsertRemote { .. } => {
                            // Metadata received. iroh-docs automatically triggers blob download.
                        }
                        LiveEvent::ContentReady { hash } => {
                            // [Temporary] Ignore pause sync
                            // let is_enabled = match manager.get_folder_config_by_doc(doc_id).await {
                            //     Some(c) => c.sync_enabled,
                            //     None => false,
                            // Map hash back to key and write to disk.
                            if let Ok(entries) = handle.list_all().await {
                                let entries: Vec<Entry> = entries;
                                if let Some(entry) = entries.iter().find(|e: &&Entry| e.content_hash() == hash) {
                                    let key = String::from_utf8_lossy(entry.key()).to_string();
                                    let target_path = root.join(&key);

                                    // 1. Check for deletion tombstone
                                    if let Ok(Some(content)) = handle.get(entry.key()).await {
                                        if content == b"__K2_SYNC_TOMBSTONE_DELETED__" {
                                            if target_path.exists() && target_path.is_file() {
                                                let _ = tokio::fs::remove_file(&target_path).await;
                                                println!("[K2-Sync] 🗑️ Remote Delete: {}", key);
                                            }
                                            continue;
                                        }
                                    }

                                    // 2. Check if local already matches
                                    if target_path.exists() {
                                        if let Ok((local_hash, _)) = manager.blob_client.add_file(target_path.clone()).await {
                                            if local_hash == hash {
                                                // Just restore mtime
                                                let meta_key = format!("meta:{}", key);
                                                if let Ok(Some(meta)) = handle.get_json::<SyncFileMeta>(meta_key.as_bytes()).await {
                                                    let ft = FileTime::from_unix_time(meta.mtime as i64, 0);
                                                    if let Ok(local_meta) = std::fs::metadata(&target_path) {
                                                        let local_mtime = FileTime::from_last_modification_time(&local_meta);
                                                        if local_mtime.unix_seconds() != ft.unix_seconds() {
                                                            let _ = filetime::set_file_mtime(&target_path, ft);
                                                            println!("[K2-Sync] 📅 Verified & restored mtime for {}: {} -> {}", key, local_mtime.unix_seconds(), ft.unix_seconds());
                                                        }
                                                    }
                                                }
                                                continue;
                                            }
                                        }
                                    }

                                    // 3. Export to disk
                                    // Ensure directory exists
                                    if let Some(parent) = target_path.parent() {
                                        let _ = tokio::fs::create_dir_all(parent).await;
                                    }
                                    
                                    if let Err(e) = manager.blob_client.export(hash, target_path.clone()).await {
                                        eprintln!("[K2-Sync] ❌ Error exporting {}: {:?}", key, e);
                                    } else {
                                        println!("[K2-Sync] ⬇️ Sync DOWN: {}", key);
                                        
                                        // 4. Restore mtime
                                        let meta_key = format!("meta:{}", key);
                                        if let Ok(Some(meta)) = handle.get_json::<SyncFileMeta>(meta_key.as_bytes()).await {
                                            let ft = FileTime::from_unix_time(meta.mtime as i64, 0);
                                            let _ = filetime::set_file_mtime(&target_path, ft);
                                            println!("[K2-Sync] 📅 Restored mtime for {}: {}", key, meta.mtime);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        Ok(())
    }
}

/// Protocol handler for receiving sync invitations (Side B)
#[derive(Clone, Debug)]
pub struct SyncInviteProtocol {
    manager: SyncManager,
}

impl SyncInviteProtocol {
    pub fn new(manager: SyncManager) -> Self {
        Self { manager }
    }
}

impl ProtocolHandler for SyncInviteProtocol {
    fn accept(&self, connection: iroh::endpoint::Connection) -> impl futures::Future<Output = std::result::Result<(), iroh::protocol::AcceptError>> + std::marker::Send {
        let manager = self.manager.clone();
        async move {
            let remote_id = connection.remote_id();
            
            println!("[K2-Sync] 📞 Incoming invitation call from {} (Bi-di)", remote_id);
            
            // 1. Trust Check (Relaxed: allow receiving, UI handles acceptance)
            /*
            let devices = match manager.list_devices().await {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("[K2-Sync] ❌ Failed to list devices for trust check: {:?}", e);
                    return Ok(());
                }
            };
            let is_trusted = devices.iter().any(|d| d.config.node_id == remote_id.to_string());
            */
            let is_trusted = true; // Temporary allow all to fix initial discovery
            
            let (mut send_stream, mut recv_stream) = match connection.accept_bi().await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[K2-Sync] ❌ Failed to accept bi-stream: {:?}", e);
                    return Ok(());
                }
            };
            
            if !is_trusted {
                println!("[K2-Sync] 🛡️ Rejecting invitation from unknown peer: {}", remote_id);
                let _ = send_stream.write_all(&[0]).await; // 0 = Reject
                let _ = send_stream.finish();
                return Ok(());
            }

            // Receive Message
            let res: Result<()> = async {
                let msg_bytes = recv_stream.read_to_end(1024).await?;
                let msg: SyncProtocolMessage = serde_json::from_slice(&msg_bytes)?;

                match msg {
                    SyncProtocolMessage::Invitation { folder_id, folder_name: _, ticket: ticket_str } => {
                        let _ticket = ticket_str.parse::<DocTicket>()
                            .map_err(|e| anyhow::anyhow!("Failed to parse ticket: {:?}", e))?;
                        
                        println!("[K2-Sync] 📩 Received ticket for folder {} from {}. Saving to pending...", folder_id, remote_id);
                        
                        // Save to pending invitations
                        let h = manager.handle().await?;
                        let invite_key = format!("pending_invite:{}", folder_id);
                        h.put(invite_key.as_bytes(), ticket_str.as_bytes()).await?;

                        // Send ACK (1)
                        let _ = send_stream.write_all(&[1]).await;
                        let _ = send_stream.finish();
                    }
                    SyncProtocolMessage::QueryStatus(folder_id) => {
                        println!("[K2-Sync] 🔍 Peer {} queried status for folder {}", remote_id, folder_id);
                        
                        // Check if we have this folder accepted
                        let folders = manager.list_folders().await?;
                        let is_accepted = folders.iter().any(|f| f.id == folder_id);
                        
                        if is_accepted {
                            let _ = send_stream.write_all(&[3]).await; // 3 = Accepted
                        } else {
                            // Check if it's still pending
                            let h = manager.handle().await?;
                            let invite_key = format!("pending_invite:{}", folder_id);
                            if h.get(invite_key.as_bytes()).await?.is_some() {
                                let _ = send_stream.write_all(&[2]).await; // 2 = Pending
                            } else {
                                // Check if we left (Soft delete tombstone)
                                let left_key = format!("left_folder:{}", folder_id);
                                if h.get(left_key.as_bytes()).await?.is_some() {
                                    let _ = send_stream.write_all(&[4]).await; // 4 = Left
                                } else {
                                    let _ = send_stream.write_all(&[1]).await; // 1 = Reject
                                }
                            }
                        }
                        let _ = send_stream.finish();
                    }
                }
                Ok(())
            }.await;

            // Wait a short moment before closing the connection to ensure the ACK is delivered
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            if let Err(e) = res {
                eprintln!("[K2-Sync] ❌ Handshake error with {}: {:?}", remote_id, e);
            } else {
                println!("[K2-Sync] ✅ Handshake completed with {}", remote_id);
            }
            
            Ok(())
        }
    }
}
