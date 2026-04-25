use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::docs::{K2DocsClient, K2DocHandle};
use crate::blobs::K2Blob;
use iroh_blobs::Hash;
use std::str::FromStr;

/// Cấu trúc dữ liệu Profile hoàn chỉnh
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub name: String,
    pub intro: String,
    pub description: String,
    pub avatar_hash: Option<String>,
    pub logo_hash: Option<String>,
    pub logo_light_hash: Option<String>,
}

/// ProfileManager: Quản lý việc lưu trữ và truy xuất Profile
#[derive(Clone)]
pub struct ProfileManager {
    docs_client: K2DocsClient,
    blob_client: K2Blob,
    handle: Option<K2DocHandle>,
}

impl ProfileManager {
    pub fn new(docs_client: K2DocsClient, blob_client: K2Blob) -> Self {
        Self {
            docs_client,
            blob_client,
            handle: None,
        }
    }

    /// Khởi tạo và tìm kiếm Document dành riêng cho Profile
    pub async fn init(&mut self) -> Result<()> {
        let mut found_id = None;
        let docs_list = self.docs_client.list_documents().await?;

        // Duyệt qua các doc để tìm profile marker
        for id in docs_list {
            if let Some(h) = self.docs_client.open_doc(id).await? {
                if let Ok(Some(marker)) = h.get(b"__k2_profile_marker__").await {
                    if marker == b"k2-profile-v1" {
                        // KIỂM TRA QUYỀN GHI: Thử ghi lại marker để xem có lỗi permission không
                        if h.put(b"__k2_profile_marker__", b"k2-profile-v1").await.is_ok() {
                            println!("[Profile] ✅ Found existing writable profile doc: {}", id);
                            found_id = Some(id);
                            break;
                        } else {
                            println!("[Profile] ⚠️ Found profile doc {} but it is READ-ONLY, skipping...", id);
                        }
                    }
                }
            }
        }

        let handle = if let Some(id) = found_id {
            self.docs_client.open_doc(id).await?.unwrap()
        } else {
            // Tạo mới nếu chưa có hoặc cái cũ không ghi được
            println!("[Profile] ✨ Creating new profile document...");
            let h = self.docs_client.create_doc().await?;
            h.put(b"__k2_profile_marker__", b"k2-profile-v1").await?;
            h
        };

        self.handle = Some(handle);
        Ok(())
    }

    fn handle(&self) -> Result<&K2DocHandle> {
        self.handle.as_ref().ok_or_else(|| anyhow::anyhow!("ProfileManager chưa được khởi tạo"))
    }

    /// Lấy thông tin Profile hiện tại
    pub async fn get(&self) -> Result<Profile> {
        let h = self.handle()?;
        
        let name = h.get(b"profile:name").await?
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();
            
        let intro = h.get(b"profile:intro").await?
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();
            
        let description = h.get(b"profile:description").await?
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();
            
        let avatar_hash = h.get(b"profile:avatar").await?
            .map(|b| String::from_utf8_lossy(&b).to_string());
            
        let logo_hash = h.get(b"profile:logo").await?
            .map(|b| String::from_utf8_lossy(&b).to_string());
            
        let logo_light_hash = h.get(b"profile:logo_light").await?
            .map(|b| String::from_utf8_lossy(&b).to_string());

        Ok(Profile {
            name,
            intro,
            description,
            avatar_hash,
            logo_hash,
            logo_light_hash,
        })
    }

    /// Cập nhật các thông tin văn bản
    pub async fn update_info(&self, name: Option<String>, intro: Option<String>, description: Option<String>) -> Result<()> {
        let h = self.handle()?;
        if let Some(n) = name {
            if !n.is_empty() {
                h.put(b"profile:name", n.as_bytes()).await?;
            }
        }
        if let Some(i) = intro {
            if !i.is_empty() {
                h.put(b"profile:intro", i.as_bytes()).await?;
            }
        }
        if let Some(d) = description {
            if !d.is_empty() {
                h.put(b"profile:description", d.as_bytes()).await?;
            }
        }
        Ok(())
    }

    /// Cập nhật Avatar (lưu vào blobs rồi lưu hash vào docs)
    pub async fn update_avatar(&self, bytes: Vec<u8>) -> Result<String> {
        let hash = self.blob_client.add_bytes(bytes).await?;
        let hash_str = hash.to_string();
        self.handle()?.put(b"profile:avatar", hash_str.as_bytes()).await?;
        Ok(hash_str)
    }

    /// Cập nhật Logo (Dark)
    pub async fn update_logo(&self, bytes: Vec<u8>) -> Result<String> {
        let hash = self.blob_client.add_bytes(bytes).await?;
        let hash_str = hash.to_string();
        self.handle()?.put(b"profile:logo", hash_str.as_bytes()).await?;
        Ok(hash_str)
    }

    /// Cập nhật Logo (Light)
    pub async fn update_logo_light(&self, bytes: Vec<u8>) -> Result<String> {
        let hash = self.blob_client.add_bytes(bytes).await?;
        let hash_str = hash.to_string();
        self.handle()?.put(b"profile:logo_light", hash_str.as_bytes()).await?;
        Ok(hash_str)
    }

    /// Lấy bytes của ảnh từ blobs dựa trên hash
    pub async fn get_image_bytes(&self, hash_str: &str) -> Result<Vec<u8>> {
        let hash = Hash::from_str(hash_str).context("Hash không hợp lệ")?;
        self.blob_client.get_bytes(hash).await
    }
}
