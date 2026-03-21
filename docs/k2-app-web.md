# k2-app-web

Web frontend cho K2 Network — P2P marketplace với AI-assisted negotiation.

## Tech Stack

- **React 19** + TypeScript 5.6
- **Vite 6** (dev server, bundler)
- **Groq LLM** (AI chat + intent classification, client-side)
- **WebSocket** (server-push realtime events)
- **@tambo-ai/react** (Tambo AI integration)
- **react-markdown** (render markdown trong chat)
- **zod** (schema validation)

## Quick Start

```bash
cd k2-app-web
cp .env.example .env     # điền VITE_GROQ_API_KEY
npm install
npm run dev              # http://localhost:5173
```

Dev server proxy `/api` và `/ws` → `http://localhost:3001` (k2-web-server).

## Environment Variables

| Variable | Default | Mô tả |
|---|---|---|
| `VITE_API_URL` | _(rỗng)_ | Backend base URL. Rỗng = dùng Vite proxy |
| `VITE_WS_URL` | _(rỗng)_ | WebSocket URL. Rỗng = dùng Vite proxy |
| `VITE_GROQ_API_KEY` | — | Groq API key (client-side) |
| `VITE_GROQ_MODEL` | `llama-3.3-70b-versatile` | Model chat chính |
| `VITE_GROQ_SMALL_MODEL` | `llama-3.1-8b-instant` | Model phân loại intent |
| `VITE_GROQ_BASE_URL` | `https://api.groq.com/openai/v1` | Groq API base URL |

## Cấu trúc thư mục

```
k2-app-web/src/
├── main.tsx                  # React entry point
├── App.tsx                   # Root component, layout, tab routing
├── api/                      # HTTP/WS clients
│   ├── client.ts             # apiFetch() wrapper
│   ├── node.ts               # initNode, getMyNodeId
│   ├── marketplace.ts        # topic join/broadcast/listen, web matching, SESSION_ID
│   ├── contacts.ts           # CRUD contacts + pingContact
│   ├── chat.ts               # sendChatMessage, sendP2pMessage, startDmListener
│   ├── files.ts              # shareFileBytes, downloadFileUrl
│   ├── ai.ts                 # classifyIntent, groqChatWithTools, generateQrSvg
│   ├── tracker.ts            # announceTopic, getTopicPeers
│   ├── ws.ts                 # K2WebSocketClient singleton (k2ws)
│   └── index.ts              # re-export tất cả
├── components/
│   ├── Header/               # Header (tiêu đề trang, node ID, username)
│   ├── Sidebar/              # Navigation 4 tab
│   ├── Chat/
│   │   ├── ChatInterface.tsx # AI assistant sidebar (resizable)
│   │   └── StartTransactionButton.tsx
│   └── DynamicForm/
│       ├── DynamicRequestForm.tsx   # Form render theo topic type
│       ├── DiscoveryView.tsx        # P2P candidate discovery
│       ├── NegotiationDashboard.tsx # Multi-candidate scoring
│       ├── CandidateList.tsx / CandidateCard.tsx
│       ├── MarketplaceTabs.tsx
│       └── SkeletonField.tsx
├── pages/
│   ├── Auth/AuthGate.tsx     # Login / register / guest
│   ├── Marketplace/          # Browse categories, đăng offer
│   ├── Negotiation/          # Chat + P2P negotiation
│   ├── Contact/              # Quản lý contacts + P2P ping
│   └── Profile/              # Thông tin user, stats radar
├── context/
│   └── AuthContext.tsx       # Auth state (pending/guest/auth, JWT, sessionId)
├── hooks/
│   └── useGroqChat.ts        # Groq chat + intent classification
├── services/
│   └── groqStructuredOutput.ts
└── tambo/                    # Tambo AI (config, tools, components)
```

## Pages & Navigation

4 tab chính trong `AppShell`:

| Tab | Page | Mô tả |
|---|---|---|
| **Marketplace** | `Marketplace.tsx` | Browse Digital Assets / Goods / Freelance; đăng và tìm offer |
| **Negotiation** | `NegotiationChat.tsx` | Chat P2P và negotiation với candidates |
| **Contact** | `Contact.tsx` | Quản lý contacts theo node ID; P2P ping |
| **Profile** | `Profile.tsx` | Thông tin user, node ID, stats radar |

### Authentication Flow

`AuthGate` hiện khi `mode === "pending"`:

1. **Guest** — không đăng nhập, giới hạn 2 trade requests (lưu trong `localStorage["k2_guest_count"]`)
2. **Login** — username + password → JWT
3. **Register** — tạo tài khoản

Refresh token lưu ở `localStorage["k2_refresh_token"]`. Khi mount, `AuthContext` tự thử restore session qua `POST /api/auth/refresh`.

`sessionId`: auth mode dùng `userId` từ JWT; guest mode dùng UUID được tạo khi module marketplace.ts load (in-memory, reset khi refresh trang).

## API Layer

Tất cả HTTP calls qua `apiFetch()` trong [src/api/client.ts](src/api/client.ts) — fetch wrapper đơn giản, throw nếu `!res.ok`.

### Endpoints

| Module | Hàm | Method | Path |
|---|---|---|---|
| node | `initNode()` | POST | `/api/init` |
| node | `getMyNodeId()` | GET | `/api/node-id` |
| contacts | `listContacts()` | GET | `/api/contacts` |
| contacts | `addContact(node_id, nickname, notes?)` | POST | `/api/contacts` |
| contacts | `removeContact(nodeId)` | DELETE | `/api/contacts/:nodeId` |
| contacts | `updateContactNickname(nodeId, nickname)` | PUT | `/api/contacts/:nodeId` |
| contacts | `pingContact(nodeId)` → `boolean` | POST | `/api/contacts/:nodeId/ping` |
| marketplace | `joinTopic(topic, action)` | POST | `/api/topics/join` |
| marketplace | `broadcastOffer(topic, form_data)` | POST | `/api/topics/broadcast` |
| marketplace | `sendInterest(topic, seller_node_id, form_data)` | POST | `/api/topics/interest` |
| marketplace | `listenOffers(topic, timeout?)` | GET | `/api/topics/offers` |
| marketplace | `startListening(topic)` | POST | `/api/topics/listen` |
| marketplace | `postOffer(topic, action, form_data)` | POST | `/api/offers` |
| marketplace | `getOffers(topic?)` | GET | `/api/offers` |
| chat | `sendChatMessage(recipient, content, session, name)` | POST | `/api/chat/send` |
| chat | `sendP2pMessage(recipient, content, session, name)` | POST | `/api/chat/send-p2p` |
| chat | `startDmListener(contact_node_id)` | POST | `/api/chat/listen` |
| files | `shareFileBytes(file)` → `{ticket, filename}` | POST | `/api/files/share` |
| files | `downloadFileUrl(ticket)` → URL string | — | `/api/files/download?ticket=` |
| ai | `classifyIntent(params)` | POST | `/api/classify-intent` |
| ai | `groqChatWithTools(params)` | POST | `/api/groq-chat` |
| ai | `classifyK2Endpoint(prompt)` | POST | `/api/k2-endpoint` |
| ai | `generateQrSvg(data)` → SVG string | POST | `/api/qr-svg` |
| tracker | `announceTopic(topic, nodeId)` | POST | `/api/tracker/announce` |
| tracker | `getTopicPeers(topic)` → `TopicPeersResponse` | GET | `/api/tracker/peers?topic=` |
| auth | _(fetch trực tiếp trong AuthContext)_ | POST | `/api/auth/refresh` |
| auth | _(fetch trực tiếp trong AuthContext)_ | POST | `/api/auth/logout` |

### WebSocket (`k2ws`)

Singleton `K2WebSocketClient` — `GET /ws?session_id=<id>`, **server-push only** (frontend không gửi gì).

```typescript
import { k2ws } from './api';

k2ws.connect(undefined, sessionId);      // kết nối / reconnect với session
k2ws.setSession(sessionId);              // cập nhật session_id sau khi có node_id
k2ws.listen('k2://offer-received', handler);  // đăng ký nhận event
```

Events nhận được (map từ server `type` → K2 event name):

| Server type | K2 event | Payload |
|---|---|---|
| `chat_message` | `k2://chat-message` | `{ recipient_session_id, payload }` — chỉ forward đến đúng session |
| `offer_received` | `k2://offer-received` | offer JSON từ gossip topic |
| `peer_connected` | `k2://peer-connected` | `{ node_id }` |
| `peer_disconnected` | `k2://peer-disconnected` | `{ node_id }` |

Auto-reconnect sau 3 giây nếu mất kết nối.

## Content Tracker

Backend tự động announce + bootstrap khi join/listen topic. Frontend có thể dùng `getTopicPeers()` để hiển thị số node đang online trong topic.

```typescript
import { getTopicPeers } from './api';

const { peers, count } = await getTopicPeers('Digital Assets');
// peers: [{ node_id: string, announced_at: number }]
```

Flow:
1. `joinTopic()` / `startListening()` → backend tự announce node vào tracker
2. `startListening()` → backend query tracker lấy peers → bootstrap gossip
3. Peers chỉ có `PublicKey`; iroh endpoint tự resolve địa chỉ qua pkarr/DHT

## AI Integration (`useGroqChat`)

Hook trong [src/hooks/useGroqChat.ts](src/hooks/useGroqChat.ts). Mỗi message qua 3 bước:

### Bước 1 — Intent classification (small model)
Gọi `POST /api/classify-intent` với small model (`llama-3.1-8b-instant`). Trả về JSON:
```typescript
interface IntentResult {
  action: 'buy' | 'sell' | 'exchange' | 'none';
  topic: 'Digital Assets' | 'Goods' | 'Freelance Job' | null;
  subtopic?: string;
  category?: string;  // cho Freelance Job
  skill?: string;
  title: string;
  description: string;
  needs_search: boolean;
}
```
Validation + fuzzy match tại client trước khi dùng (retry tối đa 2 lần).

### Bước 2 — Tool execution
- `needs_search = true` → `executeSearch()`: gọi `startListening` + `listenOffers` trên topic, filter theo keyword
- `needs_search = false` và `action !== 'none'` → `executePrepareForm()`: dispatch 2 custom DOM events:
  - `k2:showDynamicForm` — điền form marketplace
  - `k2:showStartButton` — hiện nút action

### Bước 3 — Chat response (main model)
Gọi `POST /api/groq-chat` với `llama-3.3-70b-versatile`, kết quả tool được inject vào system message `[Kết quả action]: ...`.

## State Management

| Storage | Key | Giá trị |
|---|---|---|
| `localStorage` | `k2_refresh_token` | JWT refresh token |
| `localStorage` | `k2_guest_count` | Số guest requests đã dùng (0–2) |
| `localStorage` | `k2-chat-width` | Chiều rộng chat panel (px) |
| In-memory (module) | `SESSION_ID` | UUID cho session hiện tại (reset khi reload) |
| `AuthContext` | — | `mode`, `user`, `sessionId`, `guestRequestCount` |

## Build

```bash
npm run build    # tsc + vite bundle → dist/
npm run preview  # serve dist/ locally
```
