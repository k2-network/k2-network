//! K2 Core - P2P Marketplace Library
//!
//! Built on iroh 0.95 with gossip support for marketplace trading
//!
//! Features:
//! - Contact book management
//! - P2P file sharing via iroh-blobs
//! - Marketplace gossip for trading

use anyhow::{Context, Result};
use iroh::{
    discovery::pkarr::dht::DhtDiscovery,
    protocol::Router,
    Endpoint, SecretKey,
};
use iroh_blobs::{store::mem::MemStore, BlobsProtocol, ticket::BlobTicket};
use iroh_gossip::{
    net::{Gossip, GOSSIP_ALPN},
    proto::TopicId,
    api::GossipTopic,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

// ============================================
// CONTACT BOOK
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
// K2 NODE - Main P2P Node
// ============================================

/// K2Node wraps iroh Endpoint + iroh-blobs + iroh-gossip
/// Built on iroh 0.95 with Pkarr DHT discovery
#[derive(Clone)]
pub struct K2Node {
    endpoint: Endpoint,
    blobs: BlobsProtocol,
    store: MemStore,
    gossip: Gossip,
    secret_key: SecretKey,
    #[allow(dead_code)]
    router: Arc<Router>,
}

impl K2Node {
    /// Create a new Iroh node with Pkarr DHT discovery and gossip support
    pub async fn new() -> Result<Self> {
        // Generate secret key for this node
        let secret_key = SecretKey::generate(&mut rand::rng());
        
        // Create DHT discovery builder
        let discovery = DhtDiscovery::builder()
            .n0_dns_pkarr_relay()
            .dht(true)
            .include_direct_addresses(true)
            .secret_key(secret_key.clone())
            .build()
            .context("Failed to build DHT discovery")?;
        
        // Create endpoint with blobs and gossip ALPNs
        let endpoint = Endpoint::builder()
            .secret_key(secret_key.clone())
            .discovery(discovery)
            .alpns(vec![iroh_blobs::ALPN.to_vec(), GOSSIP_ALPN.to_vec()])
            .bind()
            .await
            .context("Failed to create endpoint")?;
        
        // Create gossip
        let gossip = Gossip::builder().spawn(endpoint.clone());
        
        // Create in-memory blob store
        let store = MemStore::new();
        let blobs = BlobsProtocol::new(&store, None);
        
        // Build router with both protocols
        let router = Router::builder(endpoint.clone())
            .accept(iroh_blobs::ALPN, blobs.clone())
            .accept(GOSSIP_ALPN, gossip.clone())
            .spawn();
        
        Ok(Self {
            endpoint,
            blobs,
            store,
            gossip,
            secret_key,
            router: Arc::new(router),
        })
    }

    /// Get our public key as a string
    pub fn my_id(&self) -> String {
        self.secret_key.public().to_string()
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
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_delay() {
        for _ in 0..10 {
            let delay = K2Marketplace::get_broadcast_delay();
            assert!(delay >= 1000 && delay <= 4000);
        }
    }

    #[test]
    fn test_topic_to_id() {
        let topic1 = K2Marketplace::topic_to_id("Digital Assets");
        let topic2 = K2Marketplace::topic_to_id("Digital Assets");
        let topic3 = K2Marketplace::topic_to_id("Goods");
        
        assert_eq!(topic1, topic2);
        assert_ne!(topic1, topic3);
    }
}
