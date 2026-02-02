use anyhow::{Context, Result};
use iroh::{protocol::Router, Endpoint, NodeAddr, NodeId};
use iroh::discovery::pkarr::dht::DhtDiscovery;
use iroh_blobs::{store::mem::MemStore, BlobsProtocol, ticket::BlobTicket};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

/// A single contact in the address book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// Unique identifier - the iroh NodeId (public key)
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

/// K2Node wraps iroh Endpoint + iroh-blobs for P2P file sharing
/// Now with Pkarr DHT discovery for decentralized contact resolution!
#[derive(Clone)]
pub struct K2Node {
    endpoint: Endpoint,
    blobs: BlobsProtocol,
    #[allow(dead_code)]
    router: std::sync::Arc<Router>,
}

impl PartialEq for K2Node {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint.node_id() == other.endpoint.node_id()
    }
}

impl K2Node {
    /// Create a new Iroh node with Pkarr DHT discovery
    /// This enables decentralized contact lookup via BitTorrent mainline DHT!
    pub async fn new() -> Result<Self> {
        // Create DHT discovery builder
        let dht_discovery = DhtDiscovery::builder()
            .n0_dns_pkarr_relay()  // Use n0's relay for publishing
            .dht(true)              // Enable mainline DHT
            .include_direct_addresses(true);
        
        // Create an iroh endpoint with DHT discovery
        let endpoint = Endpoint::builder()
            .discovery(dht_discovery)
            .bind()
            .await
            .context("Failed to create endpoint")?;
        
        // Create in-memory blob store
        let store = MemStore::new();
        
        // Build the router with blobs protocol
        let blobs = BlobsProtocol::new(&store, None);
        let router = Router::builder(endpoint.clone())
            .accept(iroh_blobs::ALPN, blobs.clone())
            .spawn();
        
        Ok(Self { endpoint, blobs, router: std::sync::Arc::new(router) })
    }

    /// Connect to a contact by their NodeId (from address book)
    /// The DHT discovery will resolve their current address automatically
    /// Returns Ok if online, Err if offline or timeout (10 seconds)
    pub async fn connect_to_contact(&self, node_id_str: &str) -> Result<()> {
        use std::time::Duration;
        
        let node_id = NodeId::from_str(node_id_str)
            .context("Invalid NodeId format")?;
        
        // Create a minimal NodeAddr with just the NodeId
        // The discovery mechanism will fill in the actual addresses
        let addr = NodeAddr::new(node_id);
        
        // Connect with timeout - 10 seconds max to detect offline contacts
        let connect_future = self.endpoint.connect(addr, iroh_blobs::ALPN);
        
        tokio::time::timeout(Duration::from_secs(10), connect_future)
            .await
            .context("Connection timeout - contact may be offline")?
            .context("Failed to connect to contact")?;
        
        Ok(())
    }

    /// Share a file: adds it to the store and returns a Ticket string
    /// Format: filename|blob_ticket
    pub async fn share_file(&self, path: &Path) -> Result<String> {
        let contents = tokio::fs::read(path)
            .await
            .context("Failed to read file")?;
        
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.bin")
            .to_string();
        
        let tag = self.blobs.add_slice(&contents)
            .await
            .context("Failed to add file to store")?;
        
        let addr = self.endpoint.node_addr();
        let ticket = BlobTicket::new(addr, tag.hash, tag.format);
        
        Ok(format!("{}|{}", filename, ticket))
    }

    /// Share bytes directly and return a Ticket string
    /// Format: filename|blob_ticket
    pub async fn share_bytes(&self, data: &[u8], filename: &str) -> Result<String> {
        let tag = self.blobs.add_slice(data)
            .await
            .context("Failed to add data to store")?;
        
        let addr = self.endpoint.node_addr();
        let ticket = BlobTicket::new(addr, tag.hash, tag.format);
        
        Ok(format!("{}|{}", filename, ticket))
    }

    /// Download a file using a ticket
    /// Returns the original filename
    pub async fn download_file(&self, ticket_str: &str, save_dir: &Path) -> Result<String> {
        let (filename, blob_ticket_str) = ticket_str.split_once('|')
            .context("Invalid ticket format - missing filename")?;
        
        let ticket = BlobTicket::from_str(blob_ticket_str)
            .context("Invalid blob ticket format")?;
        
        let save_path = save_dir.join(filename);
        
        let _conn = self.endpoint.connect(ticket.node_addr().clone(), iroh_blobs::ALPN)
            .await
            .context("Step 1: Failed to connect to remote node")?;
        
        let downloader = self.blobs.downloader(&self.endpoint);
        
        downloader.download(ticket.hash(), vec![ticket.node_addr().node_id])
            .await
            .context("Step 2: Failed to download blob")?;
        
        let data = self.blobs.get_bytes(ticket.hash())
            .await
            .context("Step 3: Failed to read downloaded content from store")?;
        
        std::fs::write(&save_path, &data)
            .with_context(|| format!("Step 4: Failed to write file to {:?}", save_path))?;
        
        Ok(filename.to_string())
    }

    /// Get our node ID as a string - this is our permanent identity!
    /// Share this with friends to be added to their contact book
    pub fn my_id(&self) -> String {
        self.endpoint.node_id().to_string()
    }

    /// Get the node address for sharing
    pub fn node_addr(&self) -> NodeAddr {
        self.endpoint.node_addr()
    }

    /// Shutdown the node gracefully
    pub async fn shutdown(self) -> Result<()> {
        self.router.shutdown().await?;
        Ok(())
    }
}
