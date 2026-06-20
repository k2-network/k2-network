//! K2 Core - P2P Marketplace Library
//!
//! Built on iroh 1.0.0 with gossip support for marketplace trading
//!
//! Features:
//! - Contact book management (via iroh-docs for P2P sync)
//! - P2P file sharing via iroh-blobs
//! - Marketplace gossip for trading
//! - Tracker-based peer discovery (feature-gated: content-discovery)

use anyhow::{Context, Result};
use iroh::{
    endpoint::presets,
    protocol::Router,
    Endpoint, SecretKey,
};
#[cfg(feature = "content-discovery")]
use iroh::EndpointId;
use iroh_base::PublicKey;
// Address lookup services (DHT + mDNS) - iroh 1.0.0 extracted these to separate crates
// These are configured via Endpoint builder .address_lookup() method
// use iroh_mainline_address_lookup::DhtAddressLookup;
// use iroh_mdns_address_lookup::MdnsAddressLookup;
use iroh_blobs::{store::fs::FsStore, BlobsProtocol};
use iroh_gossip::{
    net::{Gossip, GOSSIP_ALPN},
    proto::TopicId,
    api::GossipTopic,
};
use iroh_docs::{
    protocol::Docs,
    NamespaceId,
    ALPN as DOCS_ALPN,
};
#[cfg(feature = "content-discovery")]
use iroh_content_discovery::{
    announce, query,
    protocol::{AbsoluteTime, Announce, AnnounceKind, Query, QueryFlags, SignedAnnounce, ALPN as DISCOVERY_ALPN},
};
#[cfg(feature = "content-discovery")]
use iroh_blobs::HashAndFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use iroh_gossip::api::GossipSender;
use tokio::sync::Mutex as TokioMutex;

mod identity;
mod docs;
mod blobs;
mod profile;
mod sync;

pub use identity::*;
pub use docs::*;
pub use blobs::*;
pub use profile::*;
pub use sync::*;

pub mod llm;
pub mod capabilities;
pub mod wasm;
pub mod security;
pub mod store;
pub mod agent_loop;
pub mod approval;
pub mod tools;
pub mod p2p_security;

// Default tracker ID (same as example 12)
#[cfg(feature = "content-discovery")]
pub const DEFAULT_TRACKER: &str = "71853750efc1219d7976639087c5fb25cf8d4b49f6d509366f2e094a3f781623";

// ============================================
// CONTACT BOOK (Legacy JSON - for backwards compatibility)
// ============================================

/// A single contact in the address book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// Unique identifier - the iroh public key (hex)
    pub node_id: String,
    /// User-friendly nickname
    pub nickname: String,
    /// When this contact was added (unix timestamp)
    pub added_at: u64,
    /// Optional notes
    pub notes: Option<String>,
}

/// Contact book - stores contacts locally
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContactBook {
    pub contacts: HashMap<String, Contact>,
}

impl ContactBook {
    pub fn new() -> Self {
        Self {
            contacts: HashMap::new(),
        }
    }

    /// Load from JSON file
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read_to_string(path)?;
        let book: ContactBook = serde_json::from_str(&data)?;
        Ok(book)
    }

    /// Save to JSON file
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Add a contact
    pub fn add(&mut self, node_id: String, nickname: String, notes: Option<String>) -> Contact {
        let contact = Contact {
            node_id: node_id.clone(),
            nickname,
            added_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            notes,
        };
        self.contacts.insert(node_id, contact.clone());
        contact
    }

    /// Remove a contact by node_id
    pub fn remove(&mut self, node_id: &str) -> bool {
        self.contacts.remove(node_id).is_some()
    }

    /// Get a contact by node_id
    pub fn get(&self, node_id: &str) -> Option<&Contact> {
        self.contacts.get(node_id)
    }

    /// List all contacts
    pub fn list(&self) -> Vec<&Contact> {
        self.contacts.values().collect()
    }

    /// Update nickname
    pub fn update_nickname(&mut self, node_id: &str, nickname: String) -> bool {
        if let Some(contact) = self.contacts.get_mut(node_id) {
            contact.nickname = nickname;
            true
        } else {
            false
        }
    }
}

// ============================================
// CONTACT BOOK DOCS - iroh-docs based storage
// ============================================

/// Contact book powered by iroh-docs for P2P sync
/// Stores contacts in a distributed document that can sync across devices
// ============================================

/// Contact book powered by K2Docs intelligent handle
pub struct ContactBookDocs {
    client: K2DocsClient,
    handle: Option<K2DocHandle>,
}

impl ContactBookDocs {
    pub fn new(client: K2DocsClient) -> Self {
        Self { client, handle: None }
    }

    /// Initialize the contacts document
    pub async fn init(&mut self) -> Result<()> {
        // Try to find existing contacts doc or create new one
        let mut found_id = None;
        let docs_list = self.client.list_documents().await?;
        
        for id in docs_list {
            if let Some(h) = self.client.open_doc(id).await? {
                // Check marker
                if let Ok(Some(marker)) = h.get(b"__k2_contacts_marker__").await {
                    if marker == b"k2-contacts-v1" {
                        found_id = Some(id);
                        break;
                    }
                }
            }
        }

        let handle = if let Some(id) = found_id {
            let h = self.client.open_doc(id).await?.unwrap();
            println!("[K2-Docs] 📂 Opened existing contacts document");
            h
        } else {
            let h = self.client.create_doc().await?;
            h.put(b"__k2_contacts_marker__", b"k2-contacts-v1").await?;
            println!("[K2-Docs] 📝 Created new contacts document: {}", h.id());
            h
        };

        self.handle = Some(handle);
        Ok(())
    }

    fn handle(&self) -> Result<&K2DocHandle> {
        self.handle.as_ref().ok_or_else(|| anyhow::anyhow!("ContactBookDocs not initialized"))
    }

    /// Add a contact
    pub async fn add(&self, node_id: String, nickname: String, notes: Option<String>) -> Result<Contact> {
        let contact = Contact {
            node_id: node_id.clone(),
            nickname,
            added_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            notes,
        };

        let key = format!("contact:{}", node_id);
        self.handle()?.put_json(key.as_bytes(), &contact).await?;
        println!("[K2-Docs] ✅ Added contact: {}", node_id);
        Ok(contact)
    }

    /// Remove a contact
    pub async fn remove(&self, node_id: &str) -> Result<bool> {
        let key = format!("contact:{}", node_id);
        let deleted = self.handle()?.delete(key.as_bytes()).await?;
        println!("[K2-Docs] 🗑️ Removed contact: {} (deleted {} entries)", node_id, deleted);
        Ok(deleted > 0)
    }

    /// Get a contact by node_id
    pub async fn get(&self, node_id: &str) -> Result<Option<Contact>> {
        let key = format!("contact:{}", node_id);
        self.handle()?.get_json(key.as_bytes()).await
    }

    /// List all contacts
    pub async fn list(&self) -> Result<Vec<Contact>> {
        let results = self.handle()?.list_prefix(b"contact:").await?;
        let mut contacts = Vec::new();
        for (_key, value) in results {
            if let Ok(contact) = serde_json::from_slice::<Contact>(&value) {
                contacts.push(contact);
            }
        }
        Ok(contacts)
    }

    /// Update contact nickname
    pub async fn update_nickname(&self, node_id: &str, nickname: String) -> Result<bool> {
        if let Some(mut contact) = self.get(node_id).await? {
            contact.nickname = nickname;
            let key = format!("contact:{}", node_id);
            self.handle()?.put_json(key.as_bytes(), &contact).await?;
            println!("[K2-Docs] ✏️ Updated contact nickname: {}", node_id);
            return Ok(true);
        }
        Ok(false)
    }

    /// Get the namespace ID for sharing/syncing
    pub fn namespace_id(&self) -> Option<NamespaceId> {
        self.handle.as_ref().map(|h| h.id())
    }

    /// Start syncing with peers
    pub async fn start_sync(&self, peers: Vec<iroh_base::EndpointAddr>) -> Result<()> {
        self.handle()?.sync(peers).await?;
        println!("[K2-Docs] 🔄 Started contact sync");
        Ok(())
    }
}

// ============================================
// K2 NODE - Main P2P Node
// ============================================

/// K2Node wraps iroh Endpoint + iroh-blobs + iroh-gossip + iroh-docs
/// Built on iroh 1.0.0 with address_lookup discovery
/// All data is persisted to AppData (no in-memory storage)
#[derive(Clone)]
pub struct K2Node {
    endpoint: Endpoint,
    #[allow(dead_code)]
    blobs: BlobsProtocol,
    #[allow(dead_code)]
    store: FsStore,
    blob_client: K2Blob,
    gossip: Gossip,
    docs: Docs,
    docs_client: K2DocsClient,
    profile_manager: ProfileManager,
    sync_manager: SyncManager,
    secret_key: SecretKey,
    #[allow(dead_code)]
    router: Arc<Router>,
    #[allow(dead_code)]
    data_dir: PathBuf,
    /// Cache of active topic senders for broadcasting on existing subscriptions
    active_topics: Arc<TokioMutex<HashMap<TopicId, GossipSender>>>,
}

impl K2Node {
    /// Create a new Iroh node with persistent storage at AppData
    /// All data (blobs, docs) is stored on disk, nothing in RAM only
    pub async fn new() -> Result<Self> {
        let data_dir = IdentityManager::get_roaming_dir();
        Self::with_data_dir(data_dir).await
    }

    /// Create a new Iroh node with persistent data directory
    pub async fn with_data_dir(data_dir: PathBuf) -> Result<Self> {
        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create data directory")?;

        // Load existing identity or generate new one (stored in OS Secure Store + Encrypted Backup)
        let secret_key = IdentityManager::load_or_generate()
            .context("Failed to load or generate identity")?;
        
        // Build ALPN list (conditionally include content-discovery ALPN)
        #[cfg_attr(not(feature = "content-discovery"), allow(unused_mut))]
        let mut alpn_list = vec![
            iroh_blobs::ALPN.to_vec(), 
            GOSSIP_ALPN.to_vec(), 
            DOCS_ALPN.to_vec(),
            SYNC_INVITE_ALPN.to_vec(),
        ];
        #[cfg(feature = "content-discovery")]
        alpn_list.push(DISCOVERY_ALPN.to_vec());
        
        // Create endpoint with presets::N0 (includes DNS address lookup)
        // iroh 1.0.0: presets::N0 includes DHT + DNS discovery by default
        // Additional address lookup services can be added via .address_lookup()
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(secret_key.clone())
            .alpns(alpn_list)
            .bind()
            .await
            .context("Failed to create endpoint")?;
        
        // Create gossip
        let gossip = Gossip::builder().spawn(endpoint.clone());
        
        // Create persistent blob store (FsStore) at data_dir/blobs
        let blobs_path = data_dir.join("blobs");
        std::fs::create_dir_all(&blobs_path)?;
        let store = FsStore::load(&blobs_path).await
            .context("Failed to load persistent blob store")?;
        let blobs = BlobsProtocol::new(&store, None);
        
        // Create persistent docs (redb) at data_dir/docs
        let docs_path = data_dir.join("docs");
        std::fs::create_dir_all(&docs_path)?;
        let docs = Docs::persistent(docs_path)
            .spawn(endpoint.clone(), (*store).clone(), gossip.clone())
            .await
            .context("Failed to create persistent docs")?;
        
        // Create docs client (FsStore derefs to api::Store)
        let docs_client = K2DocsClient::new(docs.clone(), store.clone());
        
        // Create blobs client
        let blob_client = K2Blob::new((*store).clone(), endpoint.clone());

        // Create sync manager
        let sync_manager = SyncManager::new(docs_client.clone(), blob_client.clone(), endpoint.clone());
        sync_manager.init().await.context("Failed to initialize sync manager")?;

        // Build router with all protocols
        let router = Router::builder(endpoint.clone())
            .accept(iroh_blobs::ALPN, blobs.clone())
            .accept(GOSSIP_ALPN, gossip.clone())
            .accept(DOCS_ALPN, docs.clone())
            .accept(SYNC_INVITE_ALPN, SyncInviteProtocol::new(sync_manager.clone()))
            .spawn();
        
        // Create profile manager
        let mut profile_manager = ProfileManager::new(docs_client.clone(), blob_client.clone());
        profile_manager.init().await.context("Failed to initialize profile manager")?;

        Ok(Self {
            endpoint,
            blobs,
            store,
            blob_client,
            gossip,
            docs,
            docs_client,
            profile_manager,
            sync_manager,
            secret_key,
            router: Arc::new(router),
            data_dir,
            active_topics: Arc::new(TokioMutex::new(HashMap::new())),
        })
    }

    /// Get our public key as a string
    pub fn my_id(&self) -> String {
        self.secret_key.public().to_string()
    }


    /// Get the low-level Iroh Docs protocol instance
    pub fn iroh_docs(&self) -> &Docs {
        &self.docs
    }

    /// Get the high-level K2DocsClient
    pub fn docs(&self) -> &K2DocsClient {
        &self.docs_client
    }

    /// Create a ContactBookDocs instance
    pub fn contact_book(&self) -> ContactBookDocs {
        ContactBookDocs::new(self.docs_client.clone())
    }

    /// Access the common blob client
    pub fn blobs(&self) -> &K2Blob {
        &self.blob_client
    }

    /// Access the profile manager
    pub fn profile(&self) -> &ProfileManager {
        &self.profile_manager
    }

    /// Access the sync manager
    pub fn sync(&self) -> &SyncManager {
        &self.sync_manager
    }

    /// Connect to a peer by their public key string (hex)
    pub async fn connect_to_contact(&self, node_id_str: &str) -> Result<()> {
        let bytes = hex::decode(node_id_str).context("Invalid hex format")?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid ID length"))?;
        let public_key = PublicKey::from_bytes(&arr)?;
        
        tokio::time::timeout(
            Duration::from_secs(10),
            self.endpoint.connect(iroh_base::EndpointAddr::from(public_key), iroh_blobs::ALPN)
        )
        .await
        .context("Connection timeout")?
        .context("Failed to connect")?;
        
        Ok(())
    }

    /// Share a file and return ticket string
    pub async fn share_file(&self, path: &Path) -> Result<String> {
        let (hash, _size) = self.blob_client.add_file(path.to_path_buf()).await?;
        let ticket = self.blob_client.create_ticket(hash).await?;
        
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.bin")
            .to_string();
        
        Ok(format!("{}|{}", filename, ticket))
    }

    /// Share bytes and return ticket string
    pub async fn share_bytes(&self, data: &[u8], filename: &str) -> Result<String> {
        let hash = self.blob_client.add_bytes(data.to_vec()).await?;
        let ticket = self.blob_client.create_ticket(hash).await?;
        
        Ok(format!("{}|{}", filename, ticket))
    }

    /// Download a file using ticket
    pub async fn download_file(&self, ticket_str: &str, save_dir: &Path) -> Result<String> {
        let (filename, blob_ticket_str) = ticket_str.split_once('|')
            .context("Invalid ticket format")?;
        
        let hash = self.blob_client.download(blob_ticket_str).await?;
        let save_path = save_dir.join(filename);
        self.blob_client.export(hash, save_path).await?;
        
        Ok(filename.to_string())
    }

    /// Subscribe to a gossip topic
    pub async fn subscribe_topic(&self, topic_id: TopicId) -> Result<GossipTopic> {
        let topic = self.gossip.subscribe(topic_id, vec![]).await?;
        Ok(topic)
    }

    /// Subscribe and join a gossip topic with peers
    pub async fn join_topic(&self, topic_id: TopicId, peers: Vec<PublicKey>) -> Result<GossipTopic> {
        if peers.is_empty() {
            return self.subscribe_topic(topic_id).await;
        }
        
        match tokio::time::timeout(
            Duration::from_secs(10),
            self.gossip.subscribe_and_join(topic_id, peers)
        ).await {
            Ok(Ok(topic)) => Ok(topic),
            _ => self.subscribe_topic(topic_id).await,
        }
    }

    /// Subscribe to topic WITH tracker-based peer discovery (like example 12)
    /// 1. Query tracker for existing peers on this topic
    /// 2. Announce ourselves on tracker
    /// 3. Subscribe and join with found peers
    #[cfg(feature = "content-discovery")]
    pub async fn subscribe_topic_with_discovery(&self, topic_id: TopicId) -> Result<GossipTopic> {
        let my_id = self.secret_key.public();
        
        // Parse tracker ID
        let tracker_bytes = hex::decode(DEFAULT_TRACKER).context("Invalid tracker hex")?;
        let tracker_arr: [u8; 32] = tracker_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid tracker length"))?;
        let tracker_id = PublicKey::from_bytes(&tracker_arr)?;
        
        // Convert topic_id to hash for content discovery
        let topic_hash = HashAndFormat::raw(iroh_blobs::Hash::new(topic_id.as_bytes()));
        
        println!("[K2] 🔍 Querying tracker for peers on topic...");
        
        // Query tracker for existing peers
        let query_args = Query {
            content: topic_hash,
            flags: QueryFlags { complete: false, verified: false },
        };
        
        let mut peers = vec![];
        if let Ok(announcements) = query(&self.endpoint, tracker_id, query_args).await {
            for ann in announcements {
                if ann.host != EndpointId::from(my_id) {
                    peers.push(ann.host);
                }
            }
        }
        println!("[K2] 📡 Found {} peers from tracker", peers.len());
        
        // Announce ourselves on tracker
        let announce_msg = Announce {
            host: EndpointId::from(my_id),
            content: topic_hash,
            kind: AnnounceKind::Complete,
            timestamp: AbsoluteTime::now(),
        };
        let signed = SignedAnnounce::new(announce_msg, &self.secret_key)?;
        let _ = announce(&self.endpoint, tracker_id, signed).await;
        println!("[K2] 📢 Announced ourselves on tracker");
        
        // Convert EndpointId to PublicKey for gossip
        let peer_keys: Vec<PublicKey> = peers.iter()
            .filter_map(|nid| {
                // EndpointId wraps PublicKey
                let bytes = nid.as_bytes();
                PublicKey::from_bytes(bytes).ok()
            })
            .take(10) // Max 10 peers
            .collect();
        
        // Subscribe and join with found peers
        if peer_keys.is_empty() {
            println!("[K2] 🌐 No peers found, subscribing solo (waiting for others)...");
            self.subscribe_topic(topic_id).await
        } else {
            println!("[K2] 🤝 Joining gossip with {} peers...", peer_keys.len());
            self.join_topic(topic_id, peer_keys).await
        }
    }

    /// Subscribe to topic WITHOUT tracker (fallback when content-discovery feature is disabled)
    #[cfg(not(feature = "content-discovery"))]
    pub async fn subscribe_topic_with_discovery(&self, topic_id: TopicId) -> Result<GossipTopic> {
        println!("[K2] ℹ️ Content discovery disabled, subscribing without tracker");
        self.subscribe_topic(topic_id).await
    }

    /// Subscribe to a topic and cache the sender for later broadcasting
    /// Returns the GossipTopic (for splitting into receiver etc)
    /// The sender is cached so broadcast_message can reuse it
    pub async fn subscribe_and_cache(&self, topic_id: TopicId) -> Result<GossipTopic> {
        let topic = self.subscribe_topic(topic_id).await?;
        Ok(topic)
    }

    /// Subscribe with discovery and cache the sender for later broadcasting
    pub async fn subscribe_with_discovery_and_cache(&self, topic_id: TopicId) -> Result<GossipTopic> {
        let topic = self.subscribe_topic_with_discovery(topic_id).await?;
        Ok(topic)
    }

    /// Cache a sender for a topic (called after split())
    pub async fn cache_sender(&self, topic_id: TopicId, sender: GossipSender) {
        let mut topics = self.active_topics.lock().await;
        topics.insert(topic_id, sender);
    }

    /// Broadcast a message to a gossip topic using cached sender
    /// Falls back to creating new subscription if no cached sender exists
    pub async fn broadcast_message(&self, topic_id: TopicId, message: Vec<u8>) -> Result<()> {
        let mut topics = self.active_topics.lock().await;
        if let Some(sender) = topics.get_mut(&topic_id) {
            // Use cached sender from existing subscription
            sender.broadcast(message.into()).await?;
        } else {
            // Fallback: create new subscription (won't reach existing peers though)
            println!("[K2] ⚠️ No cached sender for topic, creating new subscription");
            let mut topic = self.subscribe_topic(topic_id).await?;
            topic.broadcast(message.into()).await?;
        }
        Ok(())
    }

    /// Shutdown the node gracefully
    pub async fn shutdown(self) -> Result<()> {
        self.router.shutdown().await?;
        Ok(())
    }
}

// ============================================
// K2 MARKETPLACE - Gossip-based Trading
// ============================================

use rand::Rng;

/// Message types for marketplace gossip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketplaceMessage {
    /// Offer from seller/exchanger
    Offer(MarketplaceOffer),
    /// Request from buyer
    Request(MarketplaceRequest),
    /// Match notification
    Match { offer_id: String, request_id: String },
}

/// A marketplace offer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceOffer {
    pub id: String,
    pub node_id: String,
    pub topic: String,
    pub action: String,
    pub subtopic: Option<String>,
    pub category: Option<String>,
    pub skill: Option<String>,
    pub title: String,
    pub description: String,
    pub price_min: u64,
    pub price_max: u64,
    pub currency: String,
    pub timestamp: u64,
}

/// A marketplace request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceRequest {
    pub id: String,
    pub node_id: String,
    pub topic: String,
    pub subtopic: Option<String>,
    pub category: Option<String>,
    pub skill: Option<String>,
    pub title: String,
    pub description: String,
    pub budget_min: u64,
    pub budget_max: u64,
    pub currency: String,
    pub timestamp: u64,
}

/// K2 Marketplace utilities
pub struct K2Marketplace;

impl K2Marketplace {
    /// Get broadcast delay - random between 1 and 4 seconds
    pub fn get_broadcast_delay() -> u64 {
        let mut rng = rand::rng();
        rng.random_range(1000..=4000)
    }

    /// Convert topic string to TopicId
    pub fn topic_to_id(topic: &str) -> TopicId {
        TopicId::from_bytes(blake3::hash(topic.as_bytes()).into())
    }

    /// Generate a unique ID
    pub fn generate_id() -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let mut rng = rand::rng();
        let random: u32 = rng.random();
        format!("K2-{:X}-{:X}", timestamp, random)
    }

    /// Serialize a marketplace message
    pub fn serialize_message(msg: &MarketplaceMessage) -> Result<Vec<u8>> {
        postcard::to_stdvec(msg).context("Failed to serialize")
    }

    /// Deserialize a marketplace message
    pub fn deserialize_message(data: &[u8]) -> Result<MarketplaceMessage> {
        postcard::from_bytes(data).context("Failed to deserialize")
    }
}

#[cfg(test)]
mod lib_tests;
