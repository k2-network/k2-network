# K2 Network — Tài liệu Kỹ thuật

## Mục lục

1. [Tổng quan](#1-tổng-quan)
2. [Kiến trúc hệ thống](#2-kiến-trúc-hệ-thống)
3. [Cấu trúc thư mục](#3-cấu-trúc-thư-mục)
4. [k2-core — Thư viện P2P](#4-k2-core--thư-viện-p2p)
5. [k2-auth-server — Authentication Service](#5-k2-auth-server--authentication-service)
6. [k2-web-server — Backend API](#6-k2-web-server--backend-api)
7. [k2-app-web — Frontend React](#7-k2-app-web--frontend-react)
8. [Luồng dữ liệu](#8-luồng-dữ-liệu)
9. [API Reference](#9-api-reference)
10. [WebSocket Events](#10-websocket-events)
11. [Cài đặt và chạy](#11-cài-đặt-và-chạy)

---

## 1. Tổng quan

K2 là một **P2P marketplace phi tập trung** được hỗ trợ bởi AI agent. Người dùng có thể mua, bán, và trao đổi tài sản số, hàng hóa, và dịch vụ freelance thông qua hệ thống đàm phán được hỗ trợ bởi AI — không cần nền tảng trung gian.

### Điểm nổi bật

| Tính năng | Mô tả |
|-----------|-------|
| **AI Marketplace** | Phân loại intent ngôn ngữ tự nhiên (mua/bán/trao đổi) |
| **P2P Gossip** | Kết nối trực tiếp qua Iroh Gossip protocol |
| **Topic Discovery** | Tham gia topic marketplace để tìm buyer/seller phù hợp |
| **Direct Messaging** | Nhắn tin P2P trực tiếp giữa các node |
| **File Sharing** | Chia sẻ file qua iroh-blobs |
| **Contact Sync** | Đồng bộ danh bạ qua iroh-docs |
| **Web Matching Engine** | Server-side matching cho web client |

---

## 2. Kiến trúc hệ thống

```
┌─────────────────────────────────────────────────────────┐
│                    k2-app-web (React)                    │
│  ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌────────┐ │
│  │Marketplace│  │Negotiation│  │ Contact  │  │Profile │ │
│  └──────────┘  └───────────┘  └──────────┘  └────────┘ │
│  ┌────────────────────┐  ┌──────────────────────────┐   │
│  │  ChatInterface     │  │  Tambo AI Integration    │   │
│  │  (AI Chat Panel)   │  │  (Intent Classification) │   │
│  └────────────────────┘  └──────────────────────────┘   │
│    │ HTTP REST + WebSocket        │ Auth (JWT)           │
└────┼──────────────────────────────┼──────────────────────┘
     │                              │
┌────▼─────────────────────┐  ┌────▼──────────────────────┐
│   k2-web-server (Axum)   │  │  k2-auth-server (Axum)    │
│   :3001                  │  │  :3002                    │
│  ┌──────┐  ┌──────────┐  │  │  /api/auth/register       │
│  │/api/*│  │/ws (WS)  │  │  │  /api/auth/login          │
│  └──────┘  └──────────┘  │  │  /api/auth/refresh        │
│  ┌─────────────────────┐ │  │  /api/auth/logout         │
│  │  require_auth       │ │  │  ┌──────────────────────┐ │
│  │  JWT middleware     │ │  │  │  SQLite DB           │ │
│  └─────────────────────┘ │  │  │  users + tokens      │ │
│  ┌─────────────────────┐ │  │  └──────────────────────┘ │
│  │  AppState (shared)  │ │  └───────────────────────────┘
│  │  offer_store        │ │
│  │  tracker_store      │ │
│  │  node (K2Node)      │ │
│  └─────────────────────┘ │
└────┬─────────────────────┘
     │
┌────▼────────────────────────────────────────────────────┐
│                    k2-core (Rust lib)                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐  │
│  │  K2Node  │  │ContactBook│ │K2Marketplace│ │K2Docs │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────┘  │
└────┬────────────────────────────────────────────────────┘
     │
┌────▼────────────────────────────────────────────────────┐
│                  Iroh P2P Network                        │
│  iroh-gossip | iroh-blobs | iroh-docs | Pkarr DHT        │
└─────────────────────────────────────────────────────────┘
```

### Technology Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 19, Vite, TypeScript |
| AI Integration | Tambo AI, Groq API (llama-3.3-70b) |
| Backend API | Rust, Axum, Tokio |
| Auth Service | Rust, Axum, SQLite (sqlx), bcrypt, JWT |
| P2P Network | Iroh 0.95 (gossip, blobs, docs) |
| Discovery | Pkarr DHT, iroh-content-discovery |
| Serialization | Postcard (P2P), serde_json (API) |
| Auth | JWT (access 1h + refresh 30d) + Guest mode |

---

## 3. Cấu trúc thư mục

```
k2-network/
├── k2-core/                    # Rust P2P library (shared)
│   └── src/
│       ├── lib.rs              # K2Node, ContactBook, K2Marketplace
│       ├── docs.rs             # K2DocsClient, K2DocHandle (iroh-docs)
│       └── lib_tests.rs        # Unit tests
│
├── k2-auth-server/             # Standalone JWT auth service (port 3002)
│   └── src/
│       └── main.rs             # register, login, refresh, logout + SQLite
│
├── k2-web-server/              # Backend Axum server (port 3001)
│   └── src/
│       ├── main.rs             # Server entrypoint, CORS, routing
│       ├── state.rs            # AppState, WsEvent, Offer, TopicPeerEntry
│       ├── middleware/
│       │   └── auth.rs         # require_auth JWT middleware
│       ├── routes/
│       │   ├── mod.rs          # Route registration
│       │   ├── auth.rs         # /api/auth/* (register, login, refresh, logout)
│       │   ├── node.rs         # /api/init, /api/node-id
│       │   ├── contacts.rs     # /api/contacts/*
│       │   ├── marketplace.rs  # /api/offers, /api/topics/*
│       │   ├── chat.rs         # /api/chat/*
│       │   ├── files.rs        # /api/files/*
│       │   ├── ai.rs           # /api/classify-intent, /api/groq-chat
│       │   ├── tracker.rs      # /api/tracker/*
│       │   └── qr.rs           # /api/qr-svg
│       └── ws/
│           ├── mod.rs          # WebSocket handler
│           └── event_bus.rs    # Broadcast event bus
│
├── k2-app-web/                 # React Web Application
│   └── src/
│       ├── App.tsx             # Root, auth flow, WS init
│       ├── api/                # HTTP + WS client layer
│       │   ├── client.ts       # Axios base client
│       │   ├── marketplace.ts  # Offers API
│       │   ├── chat.ts         # Chat API
│       │   ├── contacts.ts     # Contacts API
│       │   ├── node.ts         # Node init API
│       │   ├── tracker.ts      # Tracker API
│       │   ├── ai.ts           # AI/classify API
│       │   └── ws.ts           # WebSocket client (k2ws)
│       ├── components/
│       │   ├── Chat/           # ChatInterface, Tambo AI panel
│       │   ├── DynamicForm/    # AI-generated forms, CandidateCard/List
│       │   ├── Header/         # App header
│       │   ├── Sidebar/        # Navigation
│       │   └── SubtopicDashboard/ # Subtopic stats view
│       ├── pages/
│       │   ├── Auth/           # AuthGate (guest / register / login)
│       │   ├── Marketplace/    # Post & browse offers
│       │   ├── Negotiation/    # P2P chat + negotiation
│       │   ├── Contact/        # Contact management
│       │   └── Profile/        # User profile
│       ├── context/
│       │   └── AuthContext.tsx # Auth state (mode, user, sessionId, tokens)
│       ├── tambo/
│       │   ├── config.ts       # Tambo AI client config
│       │   ├── tools.ts        # Tambo tools (intent, form, search)
│       │   └── components.tsx  # Tambo-aware components
│       ├── hooks/
│       │   └── useGroqChat.ts  # Groq streaming chat hook
│       └── types/              # TypeScript types
│
├── docker-compose.yml
├── Dockerfile.auth             # Auth server Docker image
├── Dockerfile.backend          # Web server Docker image
├── Dockerfile.frontend         # Frontend Docker image
├── nginx.conf
└── Cargo.toml                  # Workspace root
```

---

## 4. k2-core — Thư viện P2P

### K2Node

`K2Node` là abstraction chính bao gồm toàn bộ P2P stack.

```rust
pub struct K2Node {
    pub endpoint: Endpoint,       // Iroh QUIC endpoint
    pub gossip: Gossip,           // iroh-gossip instance
    blobs: BlobsProtocol,         // iroh-blobs for file sharing
    store: MemStore,              // In-memory blob storage
    docs: Docs,                   // iroh-docs for sync
    docs_client: K2DocsClient,    // High-level docs API
    active_topics: ...,           // Cached GossipSender per topic
    dm_rx: ...,                   // Direct message receiver channel
}
```

**Protocols được đăng ký:**

| ALPN | Protocol | Dùng cho |
|------|----------|----------|
| `iroh-blobs` | BlobsProtocol | File sharing |
| `iroh-gossip` | Gossip | Topic messaging |
| `iroh-docs` | Docs | Contact sync |
| `k2/direct-message/1` | DirectMessageHandler | P2P chat |

**Các method quan trọng:**

| Method | Mô tả |
|--------|-------|
| `K2Node::new()` | Tạo node với in-memory store |
| `K2Node::with_data_dir(path)` | Tạo node với persistent docs |
| `my_id()` | Lấy public key (hex string) |
| `join_topic(topic_id, peers)` | Subscribe gossip topic |
| `broadcast_message(topic_id, data)` | Broadcast qua gossip |
| `send_direct_message(node_id, data)` | Gửi tin nhắn trực tiếp |
| `share_file(path)` | Share file, trả về ticket string |
| `download_file(ticket, dir)` | Download file từ ticket |

**Discovery:** Sử dụng Pkarr DHT với n0 DNS relay. Node ID là Ed25519 public key.

### ContactBook & ContactBookDocs

- `ContactBook` — lưu JSON local (legacy)
- `ContactBookDocs` — đồng bộ qua iroh-docs; mỗi contact được lưu với key `contact:{node_id}`. Document được tìm bằng marker `__k2_contacts_marker__`.

### K2Marketplace

Utility class cho marketplace:

```rust
K2Marketplace::topic_to_id(topic: &str) -> TopicId
// Chuyển tên topic thành blake3 hash → TopicId

K2Marketplace::serialize_message(msg) -> Vec<u8>
// Dùng postcard (compact binary serialization)

K2Marketplace::get_broadcast_delay() -> u64
// Random 1000-4000ms (chống spam/rate-limit)
```

**Message Types:**
```rust
enum MarketplaceMessage {
    Offer(MarketplaceOffer),         // Seller broadcast
    Request(MarketplaceRequest),     // Buyer broadcast
    Match { offer_id, request_id },  // Match notification
}
```

---

## 5. k2-auth-server — Authentication Service

`k2-auth-server` là một microservice độc lập chịu trách nhiệm quản lý tài khoản người dùng và phát JWT token. Chạy trên cổng `3002`.

### Database Schema (SQLite)

```sql
-- Người dùng
CREATE TABLE users (
    id           TEXT PRIMARY KEY,   -- UUID v4
    username     TEXT UNIQUE NOT NULL,
    email        TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,     -- bcrypt
    node_id      TEXT NOT NULL,      -- 64-char hex (iroh node id)
    created_at   INTEGER NOT NULL    -- Unix timestamp
);

-- Refresh tokens (cho revoke và rotation)
CREATE TABLE refresh_tokens (
    jti       TEXT PRIMARY KEY,      -- JWT ID (unique per token)
    user_id   TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
```

### JWT Strategy

| Token | TTL | Claims |
|-------|-----|--------|
| Access token | 1 giờ | `sub` (user_id), `iat`, `exp` |
| Refresh token | 30 ngày | `sub`, `jti` (revoke key), `iat`, `exp` |

- Refresh token được **rotate** mỗi lần dùng (xóa token cũ, tạo token mới).
- Logout revoke refresh token theo `jti` trong DB.

### Endpoints

| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/auth/register` | Đăng ký tài khoản mới |
| POST | `/api/auth/login` | Đăng nhập, trả về token pair |
| POST | `/api/auth/refresh` | Rotate refresh token, cấp access token mới |
| POST | `/api/auth/logout` | Revoke refresh token |

### Request / Response

**POST /api/auth/register**
```json
// Request
{ "username": "alice", "email": "alice@example.com", "password": "secret123" }

// Response 200
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "node_id": "64-char-hex",
  "username": "alice",
  "user_id": "uuid"
}
```

**POST /api/auth/login**
```json
// Request
{ "email": "alice@example.com", "password": "secret123" }
// Response: cùng cấu trúc với register
```

**POST /api/auth/refresh**
```json
// Request
{ "refresh_token": "eyJ..." }
// Response: token pair mới + user info
```

**POST /api/auth/logout**
```json
// Request
{ "refresh_token": "eyJ..." }
// Response
{ "status": "logged_out" }
```

### Biến môi trường

| Biến | Default | Mô tả |
|------|---------|-------|
| `K2_AUTH_ADDR` | `0.0.0.0:3002` | Listen address |
| `DATABASE_URL` | `sqlite://k2_auth.db` | SQLite DB path |
| `JWT_SECRET` | `k2-secret-change-in-production` | HMAC secret key |
| `ALLOWED_ORIGINS` | localhost + k2team.xyz | CORS origins |

> **Lưu ý:** Đổi `JWT_SECRET` thành giá trị bí mật mạnh trong môi trường production.

---

## 6. k2-web-server — Backend API

### Khởi động

```
K2_SERVER_ADDR=0.0.0.0:3001  # default
ALLOWED_ORIGINS=https://k2team.xyz,...
JWT_SECRET=...                # phải khớp với k2-auth-server
```

Server lắng nghe trên `0.0.0.0:3001` và serve:
- `POST /api/*` — REST API routes
- `GET /ws` — WebSocket connection

### Auth Middleware

`k2-web-server/src/middleware/auth.rs` cung cấp Axum middleware `require_auth`:

```rust
// Áp dụng cho route group cần bảo vệ:
Router::new()
    .route("/api/protected", ...)
    .layer(from_fn_with_state(state.clone(), require_auth))
```

Middleware:
1. Lấy `Authorization: Bearer <token>` từ header
2. Verify JWT với `JWT_SECRET`
3. Inject `AccessClaims { sub, exp, iat }` vào request extensions
4. Trả về `401 Unauthorized` nếu thiếu hoặc token hết hạn

### AppState

State dùng chung giữa tất cả request handlers:

```rust
pub struct AppState {
    node: Mutex<Option<K2Node>>,           // P2P node (lazy init)
    contacts: Arc<RwLock<Option<ContactBookDocs>>>,
    topic_senders: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>>,
    event_tx: broadcast::Sender<WsEvent>,  // WS broadcast channel
    offer_store: Arc<RwLock<Vec<Offer>>>,  // In-memory offers
    tracker_store: Arc<RwLock<HashMap<String, Vec<TopicPeerEntry>>>>,
    node_to_session: Arc<RwLock<HashMap<String, String>>>, // iroh_id → session_id
}
```

### Routes

#### Auth (proxied / embedded)
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/auth/register` | Đăng ký (forward tới auth-server hoặc xử lý trực tiếp) |
| POST | `/api/auth/login` | Đăng nhập |
| POST | `/api/auth/refresh` | Refresh token |
| POST | `/api/auth/logout` | Logout + revoke |

#### Node
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/init` | Khởi tạo K2Node |
| GET | `/api/node-id` | Lấy node ID của server |

#### Contacts
| Method | Path | Mô tả |
|--------|------|-------|
| GET | `/api/contacts` | Danh sách contacts |
| POST | `/api/contacts` | Thêm contact |
| DELETE | `/api/contacts/:nodeId` | Xóa contact |
| PUT | `/api/contacts/:nodeId` | Đổi nickname |
| POST | `/api/contacts/:nodeId/ping` | Ping (kiểm tra online) |

#### Marketplace — P2P Mode
| Method | Path | Mô tả |
|--------|------|-------|
| GET | `/api/broadcast-delay` | Random delay (chống spam) |
| POST | `/api/topics/join` | Subscribe gossip topic |
| POST | `/api/topics/broadcast` | Broadcast offer lên gossip |
| POST | `/api/topics/interest` | Buyer gửi interest |
| GET | `/api/topics/offers` | Lắng nghe offers (blocking) |
| POST | `/api/topics/listen` | Start background listener |

#### Marketplace — Web Matching Engine
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/offers` | Đăng offer mới (server tự match) |
| GET | `/api/offers` | Lấy danh sách offers |

#### Chat
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/chat/send` | Gửi message (WS broadcast) |
| POST | `/api/chat/send-p2p` | Gửi P2P direct message |
| POST | `/api/chat/listen` | Bắt đầu lắng nghe DM |

#### Files
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/files/share` | Share file, trả về ticket |
| GET | `/api/files/download` | Download từ ticket |

#### AI
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/classify-intent` | Phân loại intent qua Groq |
| POST | `/api/groq-chat` | Groq chat với tool calling |
| POST | `/api/k2-endpoint` | Fallback classification |

#### Tracker
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/tracker/announce` | Announce node vào topic |
| GET | `/api/tracker/peers` | Lấy peer list cho topic |
| GET | `/api/tracker/subtopic-stats` | Thống kê subtopic |

#### QR
| Method | Path | Mô tả |
|--------|------|-------|
| POST | `/api/qr-svg` | Generate QR code SVG |

### Web Matching Engine

Khi web client đăng offer (`POST /api/offers`), server:
1. Lưu offer vào `offer_store`
2. Tìm kiếm offer ngược lại (buy↔sell, cùng topic)
3. Nếu match → emit `WsEvent::OfferMatched` → broadcast qua WS tới tất cả client

---

## 7. k2-app-web — Frontend React

### Auth Flow

```
App render → AuthContext.mode === "pending"
           → AuthGate hiển thị
           → User chọn: Guest | Token Login
           → mode = "guest" | "auth"
           → AppShell init K2Node + WS
```

`AuthContext` cung cấp:
- `mode`: `"pending" | "guest" | "auth"`
- `user`: `{ nodeId, username }` hoặc null
- `sessionId`: UUID dùng để route WS messages

### WebSocket Client (`k2ws`)

Singleton WS client tại `api/ws.ts`:

```typescript
k2ws.connect(url?, sessionId, nodeId)
k2ws.setSession(sessionId, nodeId)
k2ws.on(event, handler)
k2ws.send(data)
```

WS gửi kèm `session_id` và `node_id` khi connect. Server map `node_id → session_id` để route tin nhắn chat.

### Tambo AI Integration

Tambo AI hoạt động như conductor phân loại và điều phối intent:

**Tools:**

| Tool | Mô tả |
|------|-------|
| `extract-marketplace-intent` | Phân loại prompt → topic + action |
| `create-trade-request` | Tạo marketplace request |
| `search-marketplace` | Tìm kiếm items |
| `prepare-dynamic-form` | Generate form + dispatch event |

**Flow:**
```
User nhập chat → Tambo AI
                  → gọi extract-marketplace-intent
                  → gọi POST /api/classify-intent (Groq)
                  → gọi prepare-dynamic-form
                  → dispatch window event 'k2:showDynamicForm'
                  → MarketplacePage render DynamicForm
```

### Pages

**MarketplacePage**
- Hiển thị offers từ `GET /api/offers`
- Khi submit form → `POST /api/offers` (web mode) hoặc P2P broadcast
- Lắng nghe WS events `offer_received`, `offer_matched`

**NegotiationPage**
- Luôn mounted (để giữ message state), ẩn/hiện bằng CSS `display`
- Nhận `openChatWith` prop để auto-open chat với specific peer
- Hiển thị matched candidates từ WS events

**ContactPage**
- CRUD contacts qua `/api/contacts/*`
- Ping để kiểm tra peer online

**ProfilePage**
- Hiển thị Node ID, username
- Quản lý profile settings

---

## 8. Luồng dữ liệu

### Marketplace Intent Flow (AI → Form)

```
User nhập "muốn mua iPhone 16"
    │
    ▼
ChatInterface → Tambo AI
    │
    ▼
Tool: extract-marketplace-intent
    │
    ▼
POST /api/classify-intent → Groq API
    │   (trả về: topic="Goods", action="buy", subtopic="Electronics")
    ▼
Tool: prepare-dynamic-form
    │
    ▼
window.dispatchEvent('k2:showDynamicForm', formSchema)
    │
    ▼
MarketplacePage renders DynamicForm
```

### P2P Broadcast Flow (Post Offer)

```
User submit form
    │
    ▼
POST /api/topics/join  → K2Node.join_topic(topic_id, peers)
    │                     (peers từ tracker)
    ▼
POST /api/topics/broadcast → K2Node.broadcast_message(topic_id, postcard_bytes)
    │
    ▼
Iroh Gossip propagates to all subscribed peers
    │
    ▼
Peer node: start_listening → nhận GossipEvent
    │
    ▼
Deserialize MarketplaceMessage::Offer
    │
    ▼
WsEvent::OfferReceived → broadcast::Sender
    │
    ▼
WebSocket → Frontend 'offer_received' event
```

### Web Matching Engine Flow

```
Client A: POST /api/offers { action: "sell", topic: "Goods", ... }
    │
    ▼
Server: lưu vào offer_store, tìm offers ngược lại
    │
    ▼ (nếu tìm thấy match)
WsEvent::OfferMatched { payload: { offer_a, offer_b } }
    │
    ▼
WebSocket broadcast → tất cả WS clients
    │
    ▼
Client B nhận 'offer_matched' → hiển thị trong NegotiationPage
```

### Direct Message Flow

```
User A click "Chat" với User B
    │
    ▼
POST /api/chat/send-p2p { to: node_id_B, message }
    │
    ▼
K2Node.send_direct_message(node_id_B, bytes)
    │   ALPN: k2/direct-message/1
    ▼
Iroh QUIC connection → Node B
    │
    ▼
DirectMessageHandler.accept() → mpsc channel
    │
    ▼
WsEvent::ChatMessage { recipient_session_id, payload }
    │
    ▼
WS handler filter by session_id → gửi tới User B's WS connection
```

---

## 9. API Reference

### POST /api/offers

Request body:
```json
{
  "session_id": "uuid-string",
  "topic": "Goods",
  "action": "buy",
  "form_data": {
    "title": "iPhone 16",
    "description": "...",
    "budget_min": 500,
    "budget_max": 800,
    "currency": "USD"
  }
}
```

Response:
```json
{
  "offer_id": "K2-XXXXXXXX-XXXXXXXX",
  "matched": false
}
```

### POST /api/classify-intent

Request body:
```json
{
  "prompt": "tôi muốn mua iPhone 16 giá tốt"
}
```

Response:
```json
{
  "topic": "Goods",
  "action": "buy",
  "subtopic": "Electronics",
  "keywords": ["iPhone 16"]
}
```

### POST /api/tracker/announce

Request body:
```json
{
  "topic": "Goods",
  "node_id": "hex-public-key",
  "endpoint_addr": { "id": "...", "addrs": [...] },
  "subtopic": "Electronics",
  "action": "sell"
}
```

### GET /api/tracker/peers?topic=Goods

Response:
```json
[
  {
    "node_id": "hex...",
    "endpoint_addr": { ... },
    "announced_at": 1742000000,
    "subtopic": "Electronics",
    "action": "sell"
  }
]
```

---

## 10. WebSocket Events

Client kết nối tới `ws://server/ws?session_id=UUID&node_id=HEX`.

### Sự kiện từ Server → Client

| Event type | Payload | Mô tả |
|------------|---------|-------|
| `offer_received` | `{ offer: Offer }` | Có offer mới trên gossip |
| `offer_matched` | `{ offer_a, offer_b }` | Match buy↔sell |
| `chat_message` | `{ from, message, ... }` | Tin nhắn chat đến |
| `peer_connected` | `{ node_id }` | Peer online |
| `peer_disconnected` | `{ node_id }` | Peer offline |
| `subtopic_stats_updated` | `{ topic, stats }` | Cập nhật thống kê |

`chat_message` chỉ gửi tới client có `session_id` khớp với `recipient_session_id`.

---

## 11. Cài đặt và chạy

### Chạy với Docker Compose

```bash
docker-compose up --build
```

| Service | URL |
|---------|-----|
| Frontend | `http://localhost:80` |
| Web Server | `http://localhost:3001` |
| Auth Server | `http://localhost:3002` |

### Chạy local (development)

**Auth Server:**
```bash
JWT_SECRET=my-secret DATABASE_URL=sqlite://k2_auth.db cargo run -p k2-auth-server
```

**Web Server:**
```bash
K2_SERVER_ADDR=0.0.0.0:3001 JWT_SECRET=my-secret cargo run -p k2-web-server
```

**Frontend:**
```bash
cd k2-app-web
npm install
VITE_API_BASE_URL=http://localhost:3001 npm run dev
```

> JWT_SECRET phải giống nhau giữa auth-server và web-server để middleware verify đúng token.

### Biến môi trường

**k2-web-server:**

| Biến | Default | Mô tả |
|------|---------|-------|
| `K2_SERVER_ADDR` | `0.0.0.0:3001` | Server listen address |
| `ALLOWED_ORIGINS` | localhost + k2team.xyz | CORS allowed origins |
| `JWT_SECRET` | `k2-secret-change-in-production` | JWT verify key |

**k2-auth-server:**

| Biến | Default | Mô tả |
|------|---------|-------|
| `K2_AUTH_ADDR` | `0.0.0.0:3002` | Auth server listen address |
| `DATABASE_URL` | `sqlite://k2_auth.db` | SQLite database path |
| `JWT_SECRET` | `k2-secret-change-in-production` | JWT signing key |
| `ALLOWED_ORIGINS` | localhost + k2team.xyz | CORS allowed origins |

**k2-app-web (Vite):**

| Biến | Default | Mô tả |
|------|---------|-------|
| `VITE_API_BASE_URL` | `http://localhost:3001` | Web server URL |
| `VITE_AUTH_BASE_URL` | `http://localhost:3002` | Auth server URL |
| `VITE_GROQ_API_KEY` | — | Groq API key cho AI |
| `VITE_GROQ_BASE_URL` | `https://api.groq.com/openai/v1` | Groq endpoint |
| `VITE_GROQ_SMALL_MODEL` | `llama-3.3-70b-versatile` | Groq model |

### Build production

```bash
# Tất cả Rust binaries
cargo build --release

# Frontend static files
cd k2-app-web
npm run build
# Output: k2-app-web/dist/
```
