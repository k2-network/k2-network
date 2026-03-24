# Cơ chế Match & Đàm Phán K2 Network

## Mục lục
1. [Tổng quan luồng](#1-tổng-quan-luồng)
2. [Thuật toán Matching](#2-thuật-toán-matching)
3. [Discovery Phase (Tìm kiếm)](#3-discovery-phase-tìm-kiếm)
4. [Chấm điểm & Xếp hạng](#4-chấm-điểm--xếp-hạng)
5. [Đàm phán qua P2P Chat](#5-đàm-phán-qua-p2p-chat)
6. [Sơ đồ luồng đầy đủ](#6-sơ-đồ-luồng-đầy-đủ)
7. [Cấu trúc dữ liệu](#7-cấu-trúc-dữ-liệu)

---

## 1. Tổng quan luồng

```
User đăng offer (buy/sell)
        ↓
Server tìm match ngay lập tức
        ↓
Nếu chưa có match → DiscoveryView polling
        ↓
Có match → CandidateList hiển thị danh sách
        ↓
User bấm "Bắt đầu đàm phán"
        ↓
NegotiationDashboard chạy 3 vòng chấm điểm
        ↓
Xếp hạng ứng viên theo điểm
        ↓
User mở P2P Chat với ứng viên phù hợp nhất
        ↓
Deal xong → cancel offer, dừng tìm kiếm
```

---

## 2. Thuật toán Matching

**File:** `k2-web-server/src/routes/marketplace.rs` — `POST /api/offers`

### Tiêu chí ghép nối

Khi một offer mới được đăng, server quét toàn bộ `offer_store` để tìm offer phù hợp theo **4 điều kiện đồng thời**:

| Điều kiện | Mô tả |
|---|---|
| `topic` | Phải trùng (e.g. "Digital Assets", "Video") |
| `action` | Phải **ngược nhau**: buy ↔ sell, exchange ↔ exchange |
| `session_id` | Không được là chính mình |
| `subtopic` | Phải trùng **nếu cả hai cùng có** (wildcard nếu một bên thiếu) |
| `sub_category` | Phải trùng **nếu cả hai cùng có** (wildcard nếu một bên thiếu) |

### Logic wildcard phân cấp

```
Level 1 — subtopic:
  "Electronics" matches "Electronics"   ✅
  "Electronics" matches null            ✅ (wildcard)
  "Electronics" matches "Video"         ❌

Level 2 — sub_category:
  "Smartphones" matches "Smartphones"   ✅
  "Smartphones" matches null            ✅ (wildcard)
  "Smartphones" matches "Laptops"       ❌
```

Offer không điền subtopic/sub_category sẽ match được với **bất kỳ** offer nào trong cùng topic — dùng như một offer "chung".

### Kết quả sau khi match

- **Nếu match được:** Cả hai bên nhận sự kiện `WsEvent::OfferMatched` qua WebSocket ngay lập tức. API trả về `{ status: "matched" }`.
- **Nếu chưa match:** Offer được lưu vào `offer_store` (và PostgreSQL) với TTL 5 phút. API trả về `{ status: "waiting" }`.

> Offer cũ của cùng `session_id + topic` bị xóa khi đăng offer mới, đảm bảo mỗi user chỉ có 1 offer đang hoạt động trên mỗi topic.

---

## 3. Discovery Phase (Tìm kiếm)

**File:** `k2-app-web/src/components/DynamicForm/DiscoveryView.tsx`

Khi chưa có match ngay lập tức, `DiscoveryView` chạy polling vòng lặp:

### Các giai đoạn

```
Giai đoạn 1 — Initialize (10%)
  "Để tôi tìm match phù hợp cho bạn..."
  Chờ 800ms

Giai đoạn 2 — Joining (30%)
  "Đang tham gia topic..."
  POST /api/offers → đăng offer lên server

Giai đoạn 3 — Searching (30% → 90%)
  "Đang tìm kiếm..."
  GET /api/offers?topic=X  mỗi 5 giây
  POST /api/offers (refresh)  mỗi 4 phút ← tránh hết TTL

Giai đoạn 4 — Found (100%)
  "Đã tìm thấy X ứng viên"
  Chuyển sang CandidateList
```

### Chuyển đổi offer → Candidate

Dữ liệu từ server (raw offer) được chuyển thành object `Candidate` ở frontend:

```
offer.sender_node_id  → candidate.nodeId
offer.form_data.title → candidate.title
offer.action (đảo)    → candidate.action  (buyer thấy seller và ngược lại)
offer.match_score     → candidate.matchScore  (0.8–1.0, random nếu thiếu)
offer.form_data.priceRange → candidate.priceRange
offer.form_data.location   → candidate.location
```

---

## 4. Chấm điểm & Xếp hạng

**File:** `k2-app-web/src/components/DynamicForm/NegotiationDashboard.tsx`

Sau khi user bấm "Bắt đầu đàm phán", hệ thống chạy **3 vòng chấm điểm** cho từng ứng viên (song song). Điểm tối đa là **100**.

---

### Vòng 1 — Kiểm tra kết nối mạng (max +75 điểm)

```
Điểm ban đầu = candidate.matchScore × 50

Ping peer qua API (POST /api/contacts/ping)
  → Nếu online:   +25 điểm
  → Nếu offline:  +0 điểm

Điểm tối đa sau vòng 1: 75
```

| matchScore | Điểm cơ bản | Online bonus | Tổng vòng 1 |
|---|---|---|---|
| 1.0 | 50 | +25 | 75 |
| 0.9 | 45 | +25 | 70 |
| 0.8 | 40 | +25 | 65 |
| 1.0 | 50 | +0  | 50 |

---

### Vòng 2 — So sánh giá & địa điểm (max +25 điểm)

**Điểm giá (max +15 điểm):**

```
priceMatch = 1 - |candidate_mid - user_mid| / max(user_max, 1)

Trong đó:
  candidate_mid = (candidate.priceMin + candidate.priceMax) / 2
  user_mid      = (user.priceMin + user.priceMax) / 2

Điểm giá = priceMatch × 15
```

Ví dụ:
- User muốn mua: 100–500 USD (mid = 300)
- Ứng viên bán: 200–400 USD (mid = 300)
- priceMatch = 1 - |300 - 300| / 500 = 1.0 → +15 điểm

**Điểm địa điểm (max +10 điểm):**

```
Nếu location của ứng viên chứa location của user (so sánh không phân biệt hoa thường)
  → +10 điểm
Ngược lại: +0
```

**Điểm tối đa sau vòng 2: 100**

---

### Vòng 3 — Gửi tin nhắn quan tâm (+5 điểm)

```
Điều kiện:
  - Peer online (vòng 1 pass)
  - Điểm hiện tại >= 60
  - form_data có dữ liệu

Hành động:
  POST /api/topics/interest → gửi interest message tới peer

Kết quả:
  - Thành công: +5 điểm, final = min(100, score + 5)
  - Thất bại:   điểm giữ nguyên
```

---

### AI Notes (nhận xét tự động)

Sau 3 vòng, hệ thống tự sinh nhận xét dựa trên điểm số:

| Điểm | Nhận xét chính |
|---|---|
| ≥ 80 | "Rất phù hợp! Nên liên hệ ngay." |
| ≥ 60 | "Phù hợp. Có thể thương lượng." |
| < 60 | "Cần cân nhắc thêm." |

Kèm theo các note bổ sung:
- Giá hợp lý / có thể thương lượng / vượt ngân sách
- Cùng địa điểm / khác địa điểm
- Đang online / offline
- Trạng thái active

---

### Kết quả xếp hạng

```
Candidates được sort theo negotiationScore GIẢM DẦN:

#1  Nguyễn A   Score: 95   💬 Chat ngay
#2  Trần B     Score: 78   💬 Chat
#3  Lê C       Score: 62   💬 Chat
#4  Phạm D     Score: 45   📋 Chờ
```

---

## 5. Đàm phán qua P2P Chat

**File:** `k2-app-web/src/pages/Negotiation/NegotiationChat.tsx`

### Mở chat

Khi user click "Chat" trên một ứng viên, sự kiện `k2:openChat` được dispatch:

```json
{
  "nodeId": "abc123...",
  "name": "Nguyễn A",
  "deal": {
    "title": "Short Clips Package",
    "priceMin": 200,
    "priceMax": 400,
    "currency": "USD"
  }
}
```

### Giao diện Chat

```
┌─────────────────┬────────────────────────────────────┐
│  Contacts       │  [Deal Panel — có thể thu gọn]     │
│                 │  Short Clips Package                │
│  ● Nguyễn A     │  💰 200–400 USD | 🔄 Đang đàm phán │
│    Trần B       ├────────────────────────────────────┤
│    Lê C         │                                    │
│                 │   Nguyễn A: Chào, tôi có sẵn hàng │
│                 │   Bạn: Giá có thể giảm không?      │
│                 │                                    │
│                 ├────────────────────────────────────┤
│                 │  [ Nhập tin nhắn... ]  [Gửi]       │
└─────────────────┴────────────────────────────────────┘
```

### Routing tin nhắn

```
User gửi tin nhắn
  ↓
POST /api/chat  {recipient: nodeId, content: text, sender_session_id}
  ↓
k2-web-server: route qua WebSocket đến recipient
  ↓
Người nhận: sự kiện k2://chat-message
  ↓
Filter: bỏ qua nếu sender_session_id == session của mình
  ↓
Hiển thị tin nhắn, auto-add contact nếu chưa có
```

> **Lưu ý:** Tin nhắn chat được lưu vào PostgreSQL (`chat_messages` table) theo `conversation_id = "p2p_{nodeA}_{nodeB}"`.

### Kết thúc deal

Khi user bấm **"Đã deal xong"**:
1. Dừng `DiscoveryView` polling
2. `DELETE /api/offers` — xóa offer khỏi server (và DB)
3. Xóa khỏi `tracker_store`
4. Broadcast `SubtopicStatsUpdated` để cập nhật dashboard

---

## 6. Sơ đồ luồng đầy đủ

```
┌──────────────────────────────────────────────────────────────┐
│ USER ĐĂNG OFFER                                              │
│                                                              │
│  topic="Digital Assets", action="buy"                        │
│  selection={subtopic:"Video", sub_category:"Short Clips"}    │
│  priceRange={min:200, max:500, currency:"USD"}               │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼  POST /api/offers
┌──────────────────────────────────────────────────────────────┐
│ SERVER MATCHING (marketplace.rs)                             │
│                                                              │
│  Quét offer_store:                                           │
│  - topic == "Digital Assets"?                                │
│  - action == "sell"?                                         │
│  - subtopic == "Video" (hoặc null)?                          │
│  - sub_category == "Short Clips" (hoặc null)?                │
│  - session_id != mình?                                       │
└───────────────┬──────────────────────────┬───────────────────┘
                │                          │
           MATCH FOUND                 NO MATCH
                │                          │
                ▼                          ▼
    WsEvent::OfferMatched          Lưu vào offer_store
    (cả 2 bên nhận ngay)           + PostgreSQL (persist)
                │                          │
                │                   DiscoveryView
                │                   Poll mỗi 5 giây
                │                   Refresh mỗi 4 phút
                │                          │
                └──────────┬───────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────┐
│ CANDIDATE LIST                                               │
│                                                              │
│  Convert offer → Candidate                                   │
│  matchScore = offer.match_score || random(0.8, 1.0)         │
│                                                              │
│  User click "Bắt đầu đàm phán"                              │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ NEGOTIATION DASHBOARD — 3 ROUNDS (song song mỗi candidate)  │
│                                                              │
│  Vòng 1: Ping → score = matchScore×50, online? +25          │
│  Vòng 2: Giá → +15, địa điểm → +10                         │
│  Vòng 3: Interest msg → +5 (nếu score≥60 và online)         │
│                                                              │
│  Sort DESC → Rank #1, #2, #3...                              │
│  Sinh AI Notes                                               │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ P2P CHAT                                                     │
│                                                              │
│  k2:openChat event → mở chat với ứng viên được chọn         │
│  POST /api/chat → relay qua WebSocket                        │
│  Lưu vào DB (chat_messages)                                  │
│                                                              │
│  Deal xong → DELETE /api/offers → dừng tìm kiếm             │
└──────────────────────────────────────────────────────────────┘
```

---

## 7. Cấu trúc dữ liệu

### Offer (server — Rust)

```rust
struct Offer {
    offer_id:   String,              // UUID
    session_id: String,              // UUID của user (localStorage)
    topic:      String,              // "Digital Assets", "Video", ...
    action:     String,              // "buy" | "sell" | "exchange"
    form_data:  serde_json::Value,   // Xem cấu trúc bên dưới
    timestamp:  u64,                 // Unix seconds — TTL 5 phút
}
```

### form_data (từ DynamicForm)

```json
{
  "selection": {
    "subtopic": "Video",
    "sub_category": "Short Clips",
    "skill": "Video Editing"
  },
  "title": "Cần thuê editor video ngắn",
  "description": "...",
  "priceRange": {
    "min": 200,
    "max": 500,
    "currency": "USD"
  },
  "location": "Ho Chi Minh City",
  "sender_name": "Nguyễn A",
  "match_score": 0.95
}
```

### Candidate (frontend — TypeScript)

```typescript
interface Candidate {
    nodeId:            string;          // hex public key của peer
    name:              string;          // sender_name hoặc "Peer {shortId}"
    title:             string;
    action:            "buy" | "sell" | "exchange";
    status:            "active" | string;
    matchScore:        number;          // 0.0–1.0 từ discovery
    negotiationScore?: number;          // 0–100 từ 3 vòng đàm phán
    priceRange:        { min: number; max: number; currency: string };
    location:          string;
    topic:             string;
    description:       string;
    aiNotes?:          string;          // Tự sinh sau vòng 3
}
```

### PostgreSQL — Bảng `offers` (sau khi thêm persist)

```sql
CREATE TABLE offers (
    offer_id   TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    topic      TEXT NOT NULL,
    action     TEXT NOT NULL,     -- "buy" | "sell" | "exchange"
    form_data  JSONB NOT NULL,
    timestamp  BIGINT NOT NULL    -- Unix seconds
);
```

---

## Điểm cần lưu ý

| Vấn đề | Hiện trạng |
|---|---|
| Offer TTL | 5 phút — frontend tự refresh mỗi 4 phút |
| Offer persist | Đã lưu vào PostgreSQL (sau bản cập nhật mới nhất) |
| Chat persist | Lưu vào DB theo `conversation_id = "p2p_{A}_{B}"` |
| Tracker TTL | 1 giờ — peer entry tự hết hạn |
| Double match | Có thể xảy ra nếu 2 user post đồng thời, server xử lý first-come |
| AI trong đàm phán | AI chỉ dùng để phân loại intent (Groq LLM) và sinh AI Notes — không tự đàm phán thay người dùng |
