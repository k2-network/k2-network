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

use crate::{K2DocsClient, K2DocHandle};

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
    pub linked_devices: Vec<String>,
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
}

/// The Sync Manager coordinates local FS and remote P2P document state
#[derive(Clone)]
pub struct SyncManager {
    docs_client: K2DocsClient,
    active_folders: Arc<TokioMutex<HashMap<NamespaceId, SyncFolder>>>,
    settings_handle: Arc<TokioMutex<Option<K2DocHandle>>>,
}

impl SyncManager {
    pub fn new(docs_client: K2DocsClient) -> Self {
        Self {
            docs_client,
            active_folders: Arc::new(TokioMutex::new(HashMap::new())),
            settings_handle: Arc::new(TokioMutex::new(None)),
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
            if config.sync_enabled && config.path.exists() {
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

    pub async fn add_folder_config(&self, config: SyncFolderConfig) -> Result<()> {
        let key = format!("folder:{}", config.id);
        self.handle().await?.put_json(key.as_bytes(), &config).await?;
        
        // If enabled, register it
        if config.sync_enabled && config.path.exists() {
             if let Ok(doc_id) = config.id.parse::<NamespaceId>() {
                let _ = self.register_folder(config.path, doc_id).await;
            }
        }
        Ok(())
    }

    pub async fn remove_folder_config(&self, id: &str) -> Result<()> {
        let key = format!("folder:{}", id);
        self.handle().await?.delete(key.as_bytes()).await?;
        
        // Also stop active sync if running
        if let Ok(doc_id) = id.parse::<NamespaceId>() {
            let mut active = self.active_folders.lock().await;
            active.remove(&doc_id);
        }
        Ok(())
    }

    pub async fn list_devices(&self) -> Result<Vec<SyncDeviceConfig>> {
        let results = self.handle().await?.list_prefix(b"device:").await?;
        let mut devices = Vec::new();
        for (_key, value) in results {
            if let Ok(config) = serde_json::from_slice::<SyncDeviceConfig>(&value) {
                devices.push(config);
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

    // --- ACTIVE SYNC LOGIC ---

    /// Register and start syncing a local folder with a document
    pub async fn register_folder(&self, path: PathBuf, doc_id: NamespaceId) -> Result<()> {
        let author_id = self.docs_client.default_author().await?;
        
        // Ensure path is absolute
        let abs_path = if path.is_absolute() {
            path
        } else {
            std::fs::canonicalize(&path).context("Failed to get absolute path")?
        };
        
        let folder = SyncFolder {
            path: abs_path.clone(),
            doc_id,
            author_id,
        };

        let mut folders = self.active_folders.lock().await;
        folders.insert(doc_id, folder);

        println!("[K2-Sync] 🛰️ Registered sync folder: {:?} -> {}", abs_path, doc_id);

        // 1. Initial scan and upload (Seed the doc)
        self.sync_local_to_remote(doc_id).await?;
        
        // 2. Start local file watcher
        self.spawn_watcher(doc_id).await?;
        
        // 3. Start remote event listener
        self.spawn_remote_listener(doc_id).await?;

        Ok(())
    }

    /// Scans the local folder and pushes everything to the remote doc
    pub async fn sync_local_to_remote(&self, doc_id: NamespaceId) -> Result<()> {
        let (path, handle): (PathBuf, K2DocHandle) = {
            let folders = self.active_folders.lock().await;
            let folder = folders.get(&doc_id).context("Folder not found")?;
            (folder.path.clone(), self.docs_client.open_doc(doc_id).await?.context("Doc not found")?)
        };

        println!("[K2-Sync] 🔍 Initial scan of {:?}", path);
        self.scan_dir_recursive(&path, &path, &handle).await?;
        Ok(())
    }

    async fn scan_dir_recursive(&self, root: &Path, current: &Path, handle: &K2DocHandle) -> Result<()> {
        let mut entries = tokio::fs::read_dir(current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                // Recursion
                Box::pin(self.scan_dir_recursive(root, &path, handle)).await?;
            } else {
                let rel_path = path.strip_prefix(root)?.to_string_lossy().to_string();
                let content = tokio::fs::read(&path).await?;
                
                // Smart put handles hashing and blob storage
                handle.put(rel_path.as_bytes(), content).await?;
            }
        }
        Ok(())
    }

    /// Spawn a task to watch for local file changes
    async fn spawn_watcher(&self, doc_id: NamespaceId) -> Result<()> {
        let (path, handle): (PathBuf, K2DocHandle) = {
            let folders = self.active_folders.lock().await;
            let folder = folders.get(&doc_id).context("Folder not found")?;
            (folder.path.clone(), self.docs_client.open_doc(doc_id).await?.context("Doc not found")?)
        };

        let root: PathBuf = path.clone();
        
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
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for p in event.paths {
                            if p.is_file() {
                                let rel_result = p.strip_prefix(&root);
                                if let Ok(rel_path) = rel_result {
                                    let key = rel_path.to_string_lossy().to_string();
                                    // Debounce/Sleep a bit to let the file be written completely
                                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                    
                                    if let Ok(content) = tokio::fs::read(&p).await {
                                        if let Err(e) = handle.put(key.as_bytes(), content).await {
                                            eprintln!("[K2-Sync] ❌ Error uploading {}: {:?}", key, e);
                                        } else {
                                            println!("[K2-Sync] ⬆️ Sync UP: {}", key);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Handle deletions (optional tombstone logic)
                    EventKind::Remove(_) => {
                        // TODO: Implement tombstone in iroh-docs
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

        tokio::spawn(async move {
            while let Some(event_res) = events.next().await {
                if let Ok(event) = event_res {
                    match event {
                        LiveEvent::InsertRemote { .. } => {
                            // Metadata received. iroh-docs automatically triggers blob download.
                        }
                        LiveEvent::ContentReady { hash } => {
                            // Content downloaded. Map hash back to key and write to disk.
                            if let Ok(entries) = handle.list_all().await {
                                let entries: Vec<Entry> = entries;
                                if let Some(entry) = entries.iter().find(|e: &&Entry| e.content_hash() == hash) {
                                    let key = String::from_utf8_lossy(entry.key()).to_string();
                                    let target_path = root.join(&key);
                                    
                                    if let Ok(Some(content)) = handle.get(entry.key()).await {
                                        // Ensure directory exists
                                        if let Some(parent) = target_path.parent() {
                                            let _ = tokio::fs::create_dir_all(parent).await;
                                        }
                                        // Write file
                                        if let Err(e) = tokio::fs::write(&target_path, content).await {
                                            eprintln!("[K2-Sync] ❌ Error writing {}: {:?}", key, e);
                                        } else {
                                            println!("[K2-Sync] ⬇️ Sync DOWN: {}", key);
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
