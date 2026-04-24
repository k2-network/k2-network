//! K2 Core - P2P Marketplace Library
//!
//! Built on iroh 0.95 with gossip support for marketplace trading
//!
//! Features:
//! - Contact book management (via iroh-docs for P2P sync)
//! - P2P file sharing via iroh-blobs
//! - Marketplace gossip for trading
//! - Tracker-based peer discovery

use anyhow::{Context, Result};
use iroh::{
    discovery::pkarr::dht::DhtDiscovery,
    protocol::Router,
    Endpoint, EndpointId, SecretKey,
};
use iroh_blobs::{store::mem::MemStore, BlobsProtocol, ticket::BlobTicket, HashAndFormat};
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
use iroh_content_discovery::{
    announce, query,
    protocol::{AbsoluteTime, Announce, AnnounceKind, Query, QueryFlags, SignedAnnounce, ALPN as DISCOVERY_ALPN},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use iroh_gossip::api::GossipSender;
use tokio::sync::Mutex as TokioMutex;
mod docs;
pub use docs::*;
mod identity;
pub use identity::*;

// Default tracker ID (same as example 12)
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
/// Built on iroh 0.95 with Pkarr DHT discovery
#[derive(Clone)]
pub struct K2Node {
    endpoint: Endpoint,
    blobs: BlobsProtocol,
    store: MemStore,
    gossip: Gossip,
    docs: Docs,
    docs_client: K2DocsClient,
    secret_key: SecretKey,
    #[allow(dead_code)]
    router: Arc<Router>,
    data_dir: Option<PathBuf>,
    /// Cache of active topic senders for broadcasting on existing subscriptions
    active_topics: Arc<TokioMutex<HashMap<TopicId, GossipSender>>>,
}

impl K2Node {
    /// Create a new Iroh node with Pkarr DHT discovery and gossip support
    pub async fn new() -> Result<Self> {
        Self::with_data_dir(None).await
    }

    /// Create a new Iroh node with optional persistent data directory
    pub async fn with_data_dir(data_dir: Option<PathBuf>) -> Result<Self> {
        // Load existing identity or generate new one (stored in OS Secure Store + Encrypted Backup)
        let secret_key = IdentityManager::load_or_generate()
            .context("Failed to load or generate identity")?;
        
        // Create DHT discovery builder
        let discovery = DhtDiscovery::builder()
            .n0_dns_pkarr_relay()
            .dht(true)
            .include_direct_addresses(true)
            .secret_key(secret_key.clone())
            .build()
            .context("Failed to build DHT discovery")?;
        
        // Create endpoint with blobs, gossip, docs, and discovery ALPNs
        let endpoint = Endpoint::builder()
            .secret_key(secret_key.clone())
            .discovery(discovery)
            .alpns(vec![
                iroh_blobs::ALPN.to_vec(), 
                GOSSIP_ALPN.to_vec(), 
                DISCOVERY_ALPN.to_vec(),
                DOCS_ALPN.to_vec(),
            ])
            .bind()
            .await
            .context("Failed to create endpoint")?;
        
        // Create gossip
        let gossip = Gossip::builder().spawn(endpoint.clone());
        
        // Create in-memory blob store
        let store = MemStore::new();
        let blobs = BlobsProtocol::new(&store, None);
        
        // Create docs (in-memory or persistent based on data_dir)
        let docs = if let Some(ref dir) = data_dir {
            let docs_path = dir.join("docs");
            std::fs::create_dir_all(&docs_path)?;
            Docs::persistent(docs_path)
                .spawn(endpoint.clone(), (*store).clone(), gossip.clone())
                .await
                .context("Failed to create persistent docs")?
        } else {
            Docs::memory()
                .spawn(endpoint.clone(), (*store).clone(), gossip.clone())
                .await
                .context("Failed to create memory docs")?
        };
        
        // Build router with all protocols
        let router = Router::builder(endpoint.clone())
            .accept(iroh_blobs::ALPN, blobs.clone())
            .accept(GOSSIP_ALPN, gossip.clone())
            .accept(DOCS_ALPN, docs.clone())
            .spawn();
        
        // Create docs client
        let docs_client = K2DocsClient::new(docs.clone(), store.clone());

        Ok(Self {
            endpoint,
            blobs,
            store,
            gossip,
            docs,
            docs_client,
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


    /// Get the Docs protocol instance
    pub fn docs(&self) -> &Docs {
        &self.docs
    }

    /// Create a ContactBookDocs instance
    pub fn contact_book(&self) -> ContactBookDocs {
        ContactBookDocs::new(self.docs_client.clone())
    }

    /// Connect to a peer by their public key string (hex)
    pub async fn connect_to_contact(&self, node_id_str: &str) -> Result<()> {
        let bytes = hex::decode(node_id_str).context("Invalid hex format")?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid ID length"))?;
        let public_key = iroh::PublicKey::from_bytes(&arr)?;
        
        tokio::time::timeout(
            Duration::from_secs(10),
            self.endpoint.connect(public_key, iroh_blobs::ALPN)
        )
        .await
        .context("Connection timeout")?
        .context("Failed to connect")?;
        
        Ok(())
    }

    /// Share a file and return ticket string
    pub async fn share_file(&self, path: &Path) -> Result<String> {
        let contents = tokio::fs::read(path).await.context("Failed to read file")?;
        
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.bin")
            .to_string();
        
        // Add to store
        let tag = self.store.add_slice(&contents).await.context("Failed to add to store")?;
        
        // Create ticket with endpoint addr (iroh 0.95 API)
        let addr = self.endpoint.addr();
        let ticket = BlobTicket::new(addr, tag.hash, tag.format);
        
        Ok(format!("{}|{}", filename, ticket))
    }

    /// Share bytes and return ticket string
    pub async fn share_bytes(&self, data: &[u8], filename: &str) -> Result<String> {
        let tag = self.store.add_slice(data).await.context("Failed to add to store")?;
        
        let addr = self.endpoint.addr();
        let ticket = BlobTicket::new(addr, tag.hash, tag.format);
        
        Ok(format!("{}|{}", filename, ticket))
    }

    /// Download a file using ticket
    pub async fn download_file(&self, ticket_str: &str, save_dir: &Path) -> Result<String> {
        let (filename, blob_ticket_str) = ticket_str.split_once('|')
            .context("Invalid ticket format")?;
        
        let ticket = BlobTicket::from_str(blob_ticket_str).context("Invalid ticket")?;
        let save_path = save_dir.join(filename);
        
        // Download using blobs API (iroh-blobs 0.97)
        let downloader = self.blobs.downloader(&self.endpoint);
        downloader.download(ticket.hash(), vec![ticket.addr().id])
            .await
            .context("Failed to download")?;
        
        // Read from store
        let data = self.store.get_bytes(ticket.hash()).await.context("Failed to read")?;
        std::fs::write(&save_path, &data)?;
        
        Ok(filename.to_string())
    }

    /// Subscribe to a gossip topic
    pub async fn subscribe_topic(&self, topic_id: TopicId) -> Result<GossipTopic> {
        let topic = self.gossip.subscribe(topic_id, vec![]).await?;
        Ok(topic)
    }

    /// Subscribe and join a gossip topic with peers
    pub async fn join_topic(&self, topic_id: TopicId, peers: Vec<iroh::PublicKey>) -> Result<GossipTopic> {
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
    pub async fn subscribe_topic_with_discovery(&self, topic_id: TopicId) -> Result<GossipTopic> {
        let my_id = self.secret_key.public();
        
        // Parse tracker ID
        let tracker_bytes = hex::decode(DEFAULT_TRACKER).context("Invalid tracker hex")?;
        let tracker_arr: [u8; 32] = tracker_bytes.try_into().map_err(|_| anyhow::anyhow!("Invalid tracker length"))?;
        let tracker_id = iroh::PublicKey::from_bytes(&tracker_arr)?;
        
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
        let peer_keys: Vec<iroh::PublicKey> = peers.iter()
            .filter_map(|eid| {
                // EndpointId contains PublicKey
                let bytes = eid.as_bytes();
                iroh::PublicKey::from_bytes(bytes).ok()
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
