# P2P Direct Messaging via Iroh

## Tổng quan

Thay thế cơ chế relay tin nhắn qua server bằng **Iroh P2P direct messaging** giữa các K2 nodes. Tin nhắn được gửi trực tiếp từ node này sang node khác qua giao thức Iroh, không qua trung gian.

**Luồng hoạt động mới:**
```
Browser A
  → POST /api/chat/send-p2p
  → K2Node A ──[Iroh DM_ALPN]──► K2Node B
                                   → DM bridge task
                                   → WsEvent::ChatMessage
                                   → WebSocket
                                   → Browser B
```

**So sánh với cơ chế cũ (server relay):**
```
Browser A → POST /api/chat/send → Server → WsEvent → WebSocket → Browser B
```

---

## Kiến trúc

### ALPN Protocol

Một ALPN mới được định nghĩa trong `k2-core`:

```rust
pub const DM_ALPN: &[u8] = b"k2/direct-message/1";
```

ALPN này được đăng ký trên Iroh Endpoint cùng với các protocol hiện có (blobs, gossip, docs).

### Message Format (JSON)

Tin nhắn được serialize thành JSON trước khi gửi qua P2P stream:

```json
{
  "sender_node_id": "<hex-node-id>",
  "sender_name": "Alice",
  "content": "Hello!",
  "timestamp": 1711234567890
}
```

### DM Bridge

Sau khi `POST /api/init` khởi tạo node, một background task được spawn để:
1. Nhận tin nhắn từ `dm_rx` channel (do `DirectMessageHandler` gửi vào)
2. Parse JSON payload
3. Forward vào `WsEvent::ChatMessage { recipient_session_id: my_node_id }` → broadcast qua WebSocket đến browser

---

## Thay đổi code

### 1. `k2-core/src/lib.rs`

**Thêm mới:**

- `DM_ALPN` — ALPN constant
- `DirectMessageHandler` — implement `ProtocolHandler`, nhận incoming connections, đọc bytes từ stream, gửi vào mpsc channel
- Field `dm_rx: Arc<TokioMutex<Option<mpsc::Receiver<(String, Vec<u8>)>>>>` trong `K2Node`
- Method `send_direct_message(&self, to_node_id: &str, message: &[u8]) -> Result<()>` — mở connection đến peer, gửi bytes qua bidirectional stream
- Method `take_dm_receiver(&self) -> Option<Receiver>` — lấy receiver một lần để bridge task dùng

**Cập nhật `with_data_dir`:**

```rust
// Thêm DM_ALPN vào endpoint builder
.alpns(vec![..., DM_ALPN.to_vec()])

// Thêm handler vào router
.accept(DM_ALPN, dm_handler)
```

### 2. `k2-web-server/src/routes/node.rs`

Trong `init_node`, sau khi store node vào AppState, spawn DM bridge task:

```rust
let node_clone = { /* clone K2Node ra ngoài std::sync::Mutex */ };
let my_node_id = node_clone.my_id();
if let Some(mut dm_rx) = node_clone.take_dm_receiver().await {
    let event_tx = state.event_tx.clone();
    tokio::spawn(async move {
        while let Some((sender_node_id, raw_bytes)) = dm_rx.recv().await {
            // parse JSON → WsEvent::ChatMessage { recipient_session_id: my_node_id }
        }
    });
}
```

> `recipient_session_id = my_node_id` vì WebSocket client kết nối với `?session_id=<node_id>`. Filter trong `ws/mod.rs` sẽ deliver đúng message đến browser.

### 3. `k2-web-server/src/routes/chat.rs`

Thêm handler mới `send_p2p_message`:

```rust
// POST /api/chat/send-p2p
// Body: { recipient_node_id, sender_session_id, sender_name, content }
// → serialize JSON → node.send_direct_message(recipient_node_id, bytes)
// → trả về 200 { "status": "sent_p2p" } hoặc 502 nếu peer offline
```

Route cũ `POST /api/chat/send` (server relay) vẫn được giữ nguyên để fallback.

### 4. `k2-web-server/src/routes/mod.rs`

```rust
.route("/chat/send-p2p", post(chat::send_p2p_message))
```

### 5. `k2-app-web/src/api/chat.ts`

```typescript
export const sendP2pMessage = (
  recipient_node_id: string,
  content: string,
  sender_session_id: string,
  sender_name: string,
) =>
  apiFetch<{ status: string }>('/api/chat/send-p2p', { ... });
```

### 6. `k2-app-web/src/pages/Negotiation/NegotiationChat.tsx`

Trong `handleSendMessage`, thử P2P trước, fallback về relay nếu lỗi (peer offline, timeout):

```typescript
try {
    await apiSendP2pMessage(recipient, content, sessionId, name);
} catch {
    // fallback
    await apiSendChatMessage(recipient, content, sessionId, name);
}
```

**Receive path không thay đổi** — vẫn nhận qua `k2ws.listen('k2://chat-message', handler)`.

---

## Xử lý lỗi

| Lỗi | Hành vi |
|-----|---------|
| Peer offline / timeout (10s) | `send_direct_message` trả về `Err`, frontend fallback về relay |
| Peer không có DM_ALPN | Connection bị từ chối, fallback về relay |
| JSON parse lỗi ở receiver | Dùng raw bytes làm content |
| `take_dm_receiver` gọi lần 2 | Trả về `None`, không spawn bridge task thứ 2 |

---

## Verification

1. Build backend: `cargo build -p k2-web-server`
2. Chạy: `docker-compose up`
3. Mở 2 browser tab, cả 2 gọi `POST /api/init`
4. Copy node_id từ tab A → add contact ở tab B (và ngược lại)
5. Vào NegotiationChat, chọn contact và gửi tin nhắn
6. Verify tab kia nhận được tin (P2P path → DM bridge → WebSocket)
7. Kiểm tra console: nếu không có `"P2P send failed"` → P2P thành công
