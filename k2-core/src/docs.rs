//! K2 Docs - Lớp thông minh đơn giản hóa việc sử dụng iroh-docs (v0.95)
//!
//! Thay vì quản lý thủ công từng bản ghi và hash, K2DocsClient tự động điều phối
//! giữa iroh-docs (siêu dữ liệu) và iroh-blobs (dữ liệu thực tế), 
//! cung cấp trải nghiệm giống như một Key-Value store phân tán.

use anyhow::{Context, Result};
use iroh_docs::{
    protocol::Docs,
    api::Doc,
    store::Query,
    sync::Entry,
    NamespaceId, AuthorId,
    actor::OpenState,
};
use iroh_blobs::store::fs::FsStore;
use futures::{Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};

/// K2DocsClient: "Bộ não" điều phối iroh-docs và iroh-blobs.
#[derive(Clone)]
pub struct K2DocsClient {
    docs: Docs,
    store: FsStore,
}

impl K2DocsClient {
    pub fn new(docs: Docs, store: FsStore) -> Self {
        Self { docs, store }
    }

    /// Lấy Author mặc định của node này. 
    pub async fn default_author(&self) -> Result<AuthorId> {
        self.docs.author_default().await.context("Failed to get default author")
    }

    /// Tạo một tài liệu (Namespace) mới hoàn toàn.
    pub async fn create_doc(&self) -> Result<K2DocHandle> {
        let doc = self.docs.create().await.context("Failed to create document")?;
        let author = self.default_author().await?;
        Ok(K2DocHandle::new(doc, author, self.store.clone()))
    }

    /// Mở một tài liệu đã tồn tại bằng ID.
    pub async fn open_doc(&self, id: NamespaceId) -> Result<Option<K2DocHandle>> {
        if let Some(doc) = self.docs.open(id).await? {
            let author = self.default_author().await?;
            Ok(Some(K2DocHandle::new(doc, author, self.store.clone())))
        } else {
            Ok(None)
        }
    }

    /// Liệt kê tất cả các tài liệu (NamespaceId) mà node này đang lưu trữ.
    pub async fn list_documents(&self) -> Result<Vec<NamespaceId>> {
        let mut docs = Vec::new();
        let mut stream = self.docs.list().await?;
        while let Some(result) = stream.next().await {
            if let Ok((id, _capability)) = result {
                docs.push(id);
            }
        }
        Ok(docs)
    }

    /// Join một tài liệu từ DocTicket (nhận từ peer khác).
    pub async fn import_doc(&self, ticket: iroh_docs::DocTicket) -> Result<K2DocHandle> {
        let doc = self.docs.import(ticket).await.context("Failed to import document")?;
        let author = self.default_author().await?;
        Ok(K2DocHandle::new(doc, author, self.store.clone()))
    }

    /// Xóa một tài liệu khỏi node.
    pub async fn drop_doc(&self, id: NamespaceId) -> Result<()> {
        self.docs.drop_doc(id).await.context("Failed to drop document")
    }
}

/// K2DocHandle: Một lớp thông minh đại diện cho một tài liệu cụ thể.
#[derive(Clone)]
pub struct K2DocHandle {
    inner: Doc,
    author: AuthorId,
    store: FsStore,
}

impl K2DocHandle {
    pub fn new(doc: Doc, author: AuthorId, store: FsStore) -> Self {
        Self { inner: doc, author, store }
    }

    pub fn id(&self) -> NamespaceId {
        self.inner.id()
    }

    /// THÔNG MINH: Tự động lưu dữ liệu vào Blob Store và tạo Entry trong Document.
    pub async fn put(&self, key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Result<()> {
        let k = key.into();
        let v = value.into();
        
        // Iroh v0.95+ does not allow empty entries
        if v.is_empty() {
            return Ok(());
        }

        self.inner.set_bytes(self.author, k, v).await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Iroh set_bytes error: {:?}. Author: {:?}, Doc: {:?}", e, self.author, self.inner.id()))
            .context("Failed to set bytes")
    }

    /// THÔNG MINH: Tự động trích xuất dữ liệu thực tế từ Blob Store dựa trên Hash trong Entry.
    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Some(entry) = self.inner.get_exact(self.author, key, false).await? {
            let hash = entry.content_hash();
            if let Ok(content) = self.store.get_bytes(hash).await {
                return Ok(Some(content.to_vec()));
            } else {
                return Err(anyhow::anyhow!("Blob not found in store for hash: {:?}", hash));
            }
        }
        Ok(None)
    }

    /// THÔNG MINH: Lưu trữ trực tiếp một struct có thể Serialize (JSON).
    pub async fn put_json<T: Serialize>(&self, key: impl Into<Vec<u8>>, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        self.put(key, bytes).await
    }

    /// THÔNG MINH: Lấy và Deserialze JSON trực tiếp vào struct.
    pub async fn get_json<T: DeserializeOwned>(&self, key: &[u8]) -> Result<Option<T>> {
        if let Some(bytes) = self.get(key).await? {
            let val = serde_json::from_slice(&bytes)?;
            return Ok(Some(val));
        }
        Ok(None)
    }

    /// Truy vấn danh sách entry theo Prefix của Key.
    pub async fn list_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let query = Query::key_prefix(prefix);
        let stream = self.inner.get_many(query).await?;
        tokio::pin!(stream);
        
        let mut results = Vec::new();
        while let Some(result) = stream.next().await {
            let entry: Entry = result.context("Failed to get entry from stream")?;
            let hash = entry.content_hash();
            if let Ok(content) = self.store.get_bytes(hash).await {
                results.push((entry.key().to_vec(), content.to_vec()));
            }
        }
        Ok(results)
    }

    /// Liệt kê tất cả các Entry (chỉ Metadata) trong Document này.
    pub async fn list_all(&self) -> Result<Vec<Entry>> {
        let query = Query::all();
        let stream = self.inner.get_many(query).await?;
        tokio::pin!(stream);
        
        let mut results = Vec::new();
        while let Some(result) = stream.next().await {
            let entry: Entry = result.context("Failed to get entry from stream")?;
            results.push(entry);
        }
        Ok(results)
    }

    /// THÔNG MINH: Xóa một bản ghi (tạo tombstone). 
    pub async fn delete(&self, key: impl Into<Vec<u8>>) -> Result<usize> {
        self.inner.del(self.author, key.into()).await
            .context("Failed to delete entry")
    }

    /// Lấy ID của Author đang được sử dụng trong Handle này.
    pub fn author(&self) -> AuthorId {
        self.author
    }

    /// THÔNG MINH: Lấy quyền trạng thái của tài liệu.
    pub async fn status(&self) -> Result<OpenState> {
        self.inner.status().await.context("Failed to get doc status")
    }

    /// Theo dõi các thay đổi của tài liệu trong thời gian thực.
    /// Trả về một Stream các LiveEvent. Lưu ý: Item là Result<LiveEvent>.
    pub async fn subscribe(&self) -> Result<impl Stream<Item = Result<iroh_docs::engine::LiveEvent>>> {
        let stream = self.inner.subscribe().await.context("Failed to subscribe")?;
        Ok(stream.map(|res| res.map_err(anyhow::Error::from)))
    }

    /// Bắt đầu đồng bộ hóa dữ liệu với các Peer khác.
    pub async fn sync(&self, peers: Vec<iroh_base::EndpointAddr>) -> Result<()> {
        self.inner.start_sync(peers).await
            .map(|_| ())
            .context("Failed to start sync")
    }

    /// Dừng đồng bộ tài liệu.
    pub async fn leave(&self) -> Result<()> {
        self.inner.leave().await.context("Failed to leave sync")
    }

    /// Chia sẻ tài liệu qua DocTicket.
    pub async fn share(&self, mode: iroh_docs::api::protocol::ShareMode) -> Result<iroh_docs::DocTicket> {
        self.inner.share(mode, iroh_docs::api::protocol::AddrInfoOptions::Id)
            .await.context("Failed to share document")
    }

    /// Truy vấn linh hoạt bằng Query.
    pub async fn get_one(&self, query: impl Into<iroh_docs::store::Query>) -> Result<Option<Entry>> {
        self.inner.get_one(query).await.context("Failed to get entry")
    }

    /// Truy vấn nhiều entries bằng Query.
    pub async fn get_many(&self, query: impl Into<iroh_docs::store::Query>) -> Result<impl Stream<Item = Result<Entry>>> {
        let stream = self.inner.get_many(query).await.context("Failed to get entries")?;
        Ok(stream.map(|res| res.map_err(anyhow::Error::from)))
    }
}
