//! K2 Blob - Intelligent wrapper for iroh-blobs (v0.103)
//!
//! Provides a simplified interface for adding, retrieving, sharing, and downloading blobs.

use anyhow::{Context, Result};
use iroh::Endpoint;
use iroh_blobs::{
    api::Store,
    ticket::BlobTicket,
    Hash, BlobFormat,
};
use iroh_blobs::api::blobs::{ImportMode, AddPathOptions};
use std::path::PathBuf;
use std::str::FromStr;
use futures::StreamExt;

/// K2Blob: P2P Data Management using iroh-blobs API
#[derive(Clone, Debug)]
pub struct K2Blob {
    store: Store,
    endpoint: Endpoint,
}

impl K2Blob {
    /// Initialize K2Blob with store handle and endpoint
    pub fn new(store: Store, endpoint: Endpoint) -> Self {
        Self { store, endpoint }
    }

    /// Add bytes to local store
    pub async fn add_bytes(&self, bytes: Vec<u8>) -> Result<Hash> {
        let tag = self.store.blobs().add_slice(bytes).await?;
        Ok(tag.hash)
    }

    /// Add file from path to local store using Reference mode (zero-copy)
    pub async fn add_file(&self, path: PathBuf) -> Result<(Hash, u64)> {
        println!("[K2-Blob] 🔍 Indexing file: {:?}", path);
        
        // Use add_path_with_opts to specify ImportMode::TryReference
        let options = AddPathOptions {
            path: path.clone(),
            format: BlobFormat::Raw,
            mode: ImportMode::TryReference,
        };
        let tag = self.store.blobs().add_path_with_opts(options).await?;
        
        // Get size for the hash
        let size = match self.store.blobs().status(tag.hash).await? {
            iroh_blobs::api::blobs::BlobStatus::Complete { size } => size,
            _ => 0,
        };
        
        println!("[K2-Blob] ✅ Indexed: {} (Size: {} bytes)", tag.hash, size);
        Ok((tag.hash, size))
    }

    /// Get bytes from local store by hash
    pub async fn get_bytes(&self, hash: Hash) -> Result<Vec<u8>> {
        let bytes = self.store.blobs().get_bytes(hash).await
            .context("Không thể lấy dữ liệu từ store")?;
        Ok(bytes.to_vec())
    }

    /// Export blob to a file
    pub async fn export(&self, hash: Hash, path: PathBuf) -> Result<()> {
        self.store.blobs().export(hash, path).await
            .map(|_| ()) // Convert Result<u64> to Result<()>
            .context("Không thể xuất file")
    }

    /// Create a ticket for sharing a blob
    pub async fn create_ticket(&self, hash: Hash) -> Result<String> {
        // Use iroh 1.0.0 endpoint addr API
        let addr = self.endpoint.addr();
        // BlobTicket::new returns the ticket directly (no Result)
        let ticket = BlobTicket::new(addr, hash, BlobFormat::Raw);
        Ok(ticket.to_string())
    }

    /// Download a blob from a ticket string
    pub async fn download(&self, ticket_str: &str) -> Result<Hash> {
        let ticket = BlobTicket::from_str(ticket_str).context("Invalid ticket")?;
        
        // Use downloader from store (iroh-blobs 0.103)
        let downloader = self.store.downloader(&self.endpoint);
        
        // Start download with peer list (iroh-blobs 0.103: ContentDiscovery trait)
        downloader.download(ticket.hash(), vec![ticket.addr().id])
            .await
            .context("Download failed")?;
            
        Ok(ticket.hash())
    }

    /// List all blobs in store (including their size)
    pub async fn list(&self) -> Result<Vec<(Hash, u64)>> {
        let mut blobs = Vec::new();
        // .list() returns BlobsListProgress, needs .stream().await to get stream
        let mut stream = self.store.blobs().list().stream().await?;
        while let Some(hash_result) = stream.next().await {
            let hash = hash_result?;
            // Get size for this hash using status()
            if let Ok(status) = self.store.blobs().status(hash).await {
                match status {
                    iroh_blobs::api::blobs::BlobStatus::Complete { size } => {
                        blobs.push((hash, size));
                    },
                    _ => {
                        blobs.push((hash, 0));
                    }
                }
            } else {
                blobs.push((hash, 0));
            }
        }
        Ok(blobs)
    }

    /// Check if store has a specific hash
    pub async fn has(&self, hash: &Hash) -> Result<bool> {
        Ok(self.store.blobs().get_bytes(*hash).await.is_ok())
    }

    /// Get blob size without downloading full content
    pub async fn get_size(&self, hash: &Hash) -> Result<Option<u64>> {
        match self.store.blobs().status(*hash).await {
            Ok(iroh_blobs::api::blobs::BlobStatus::Complete { size }) => Ok(Some(size)),
            Ok(iroh_blobs::api::blobs::BlobStatus::Partial { size }) => Ok(size),
            Ok(iroh_blobs::api::blobs::BlobStatus::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a blob by its hash (remove tag)
    pub async fn delete(&self, hash: &Hash) -> Result<()> {
        // iroh-blobs uses tag-based GC. We delete all tags pointing to this hash.
        let mut tags = self.store.tags().list().await?;
        while let Some(tag_result) = tags.next().await {
            let tag_info = tag_result?;
            if tag_info.hash == *hash {
                self.store.tags().delete(tag_info.name).await?;
            }
        }
        Ok(())
    }

    /// Access the underlying store (for advanced usage)
    pub fn store(&self) -> &Store {
        &self.store
    }
}
