# K2 Architecture

## Overview

K2 is a Tauri-based desktop application combining a React frontend with a Rust backend. The P2P layer is built on Iroh (v0.95), providing gossip messaging, blob storage, and document sync.

<img width="4571" height="1442" alt="image" src="https://github.com/user-attachments/assets/54ec3bca-ab03-42c6-9335-b74812626b55" />


## Project Structure

```
k2/
├── k2-core/              # Rust P2P library
│   └── src/lib.rs        # K2Node, ContactBook, Marketplace logic
├── k2-app-tauri/
│   ├── src/              # React frontend
│   │   ├── components/   # UI components (Chat, DynamicForm, Sidebar, Header)
│   │   ├── pages/        # Marketplace, Negotiation, Contact, Profile
│   │   ├── tambo/        # Tambo AI integration (tools, components, config)
│   │   └── hooks/        # Custom React hooks
│   └── src-tauri/        # Tauri backend
│       └── src/lib.rs    # Tauri commands (P2P, file sharing, marketplace)
└── Cargo.toml            # Workspace config
```

## Core Components

### k2-core (Rust Library)

Provides P2P functionality:

| Component | Description |
|-----------|-------------|
| `K2Node` | Wraps Iroh Endpoint + iroh-blobs + iroh-gossip + iroh-docs |
| `ContactBook` | JSON-based contact storage (legacy) |
| `ContactBookDocs` | iroh-docs based contact sync across devices |
| `K2Marketplace` | Topic subscription and gossip messaging |

**Dependencies**:
- `iroh` 0.95 with `discovery-pkarr-dht` feature
- `iroh-gossip` 0.95 for pub/sub messaging
- `iroh-blobs` 0.97 for file sharing
- `iroh-docs` 0.95 for document sync
- `iroh-content-discovery` for tracker-based discovery

### k2-app-tauri (Tauri Backend)

Exposes Rust functionality to the frontend via Tauri commands:

**Node Management**
- `init_node` - Initialize the P2P node
- `get_my_node_id` - Get the node's public ID

**Contact Book**
- `add_contact`, `remove_contact`, `list_contacts`, `update_contact_nickname`
- `ping_contact` - Check if a contact is online
- `send_chat_message` - Direct P2P messaging
- `start_dm_listener` - Listen for incoming messages

**File Sharing**
- `share_file`, `share_bytes` - Share files/bytes and get ticket
- `download_file` - Download using ticket
- `generate_qr_svg` - Generate QR code for sharing

**Marketplace**
- `join_topic` - Subscribe to a marketplace topic
- `broadcast_offer` - Broadcast sell offer to topic
- `send_interest` - Buyer responds to seller
- `listen_offers` - Receive offers (blocking)
- `start_listening` - Background listener with Tauri events
- `classify_intent` - AI classification via Groq API
- `classify_k2_endpoint` - Fallback classification endpoint

### React Frontend

Built with React 19, Vite, and TypeScript.

**Pages**:
- `MarketplacePage` - Create buy/sell requests
- `NegotiationPage` - View and manage negotiations
- `ContactPage` - Manage P2P contacts
- `ProfilePage` - User profile and settings

**Key Components**:
- `ChatInterface` - AI chat panel with Tambo AI
- `DynamicForm` - Form generated from AI intent classification
- `CandidateCard` - Display matched buyers/sellers

### Tambo AI Integration

Located in `src/tambo/`:

**Tools** (`tools.ts`):
- `extract-marketplace-intent` - Classify user prompt into topic/action
- `create-trade-request` - Create a marketplace request
- `search-marketplace` - Search for items
- `prepare-dynamic-form` - Generate form and dispatch to UI

**Components** (`components.tsx`):
- Custom Tambo-aware React components for marketplace UI

**Config** (`config.ts`):
- Tambo AI client configuration

## Data Flow

### Marketplace Intent Flow

```
User Prompt → ChatInterface → Tambo AI
                                ↓
                     extract-marketplace-intent tool
                                ↓
                     Tauri: classify_intent (Groq API)
                            or classify_k2_endpoint (fallback)
                                ↓
                     prepare-dynamic-form tool
                                ↓
                     window.dispatchEvent('k2:showDynamicForm')
                                ↓
                     MarketplacePage renders DynamicForm
```

### P2P Broadcast Flow

```
User submits form → MarketplacePage
                        ↓
               Tauri: join_topic (subscribe to gossip topic)
                        ↓
               Tauri: broadcast_offer (send to gossip network)
                        ↓
               Iroh Gossip propagates to peers
                        ↓
               Other nodes: start_listening receives offer
                        ↓
               Tauri event: 'k2://offer-received' → Frontend
```

## Network Architecture

- **Tracker**: Hardcoded tracker node ID for topic discovery
- **Topics**: Separate gossip topics for Digital Assets, Goods, Freelance Jobs
- **DHT Discovery**: Pkarr DHT for peer discovery
- **Message Format**: Postcard-serialized structs (compact binary)

## Technology Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 19, Vite, TypeScript |
| AI Integration | Tambo AI, Groq API |
| Desktop Framework | Tauri 2 |
| P2P Network | Iroh 0.95 (gossip, blobs, docs) |
| Backend | Rust |
| Serialization | Postcard, serde_json |
