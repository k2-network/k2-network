# K2 Network
<img width="421" height="331" alt="image" src="https://github.com/user-attachments/assets/9a03018a-7ce5-40fc-893f-821e7cee64e5" />

K2 is a decentralized P2P marketplace powered by AI agents. Users can buy, sell, and exchange digital assets, goods, and freelance services through an AI-assisted negotiation system without relying on centralized platforms.

## Features

- **AI-Powered Marketplace**: Natural language intent classification for buy/sell/exchange requests
- **P2P Communication**: Direct peer-to-peer connections via Iroh Gossip protocol
- **Contact Management**: P2P-synced contact book with direct messaging
- **File Sharing**: Share and download files directly between peers using iroh-blobs
- **Topic-Based Discovery**: Join marketplace topics to find relevant buyers/sellers
- **Authentication**: JWT-based auth (register/login/refresh/logout) with bcrypt + SQLite

## Architecture

| Component | Description | Port |
|-----------|-------------|------|
| `k2-app-web` | React frontend (Vite + TypeScript) | 80 |
| `k2-web-server` | Axum REST API + WebSocket + P2P | 3001 |
| `k2-auth-server` | JWT auth microservice (SQLite) | 3002 |
| `k2-core` | Shared Rust P2P library (Iroh) | — |

## Quick Start (Docker)

```bash
docker-compose up --build
```

Open `http://localhost` in your browser.

## Run from Source

**Prerequisites:** Rust & Cargo, Node.js (v18+)

```bash
# 1. Auth server
JWT_SECRET=my-secret cargo run -p k2-auth-server

# 2. Web server (new terminal)
JWT_SECRET=my-secret cargo run -p k2-web-server

# 3. Frontend (new terminal)
cd k2-app-web
npm install
VITE_API_BASE_URL=http://localhost:3001 npm run dev
```

## Configuration

Create `.env` file in `k2-app-web/`:

```env
VITE_API_BASE_URL=http://localhost:3001
VITE_AUTH_BASE_URL=http://localhost:3002
VITE_GROQ_API_KEY=your_groq_api_key
VITE_GROQ_BASE_URL=https://api.groq.com/openai/v1
VITE_GROQ_SMALL_MODEL=llama-3.3-70b-versatile
```

## Usage

1. Open the app and register / login (or continue as Guest)
2. Open the AI Chat panel (bottom-right)
3. Describe what you want to buy, sell, or exchange
4. The AI will classify your intent and prepare a dynamic form
5. Submit your request to broadcast on the P2P network

<img width="1918" height="991" alt="Screenshot 2026-02-08 134812" src="https://github.com/user-attachments/assets/dd504bd0-3e64-49eb-b380-7a784c2df7b4" />
<img width="1918" height="986" alt="2" src="https://github.com/user-attachments/assets/1924f82e-ba20-4c3a-9335-fa98ea86243a" />
<img width="1918" height="987" alt="3" src="https://github.com/user-attachments/assets/0f166ab5-1a21-420d-8fdf-b81c04486d28" />
<img width="1918" height="985" alt="4" src="https://github.com/user-attachments/assets/5b82dec1-7b87-4c68-a9c1-9307c2fe0c2f" />
<img width="1918" height="987" alt="5" src="https://github.com/user-attachments/assets/a255ecc7-5b7a-4f7d-a14f-18d744cff355" />
<img width="1918" height="988" alt="6" src="https://github.com/user-attachments/assets/01d676b7-b8ed-4c62-a4e5-1da1188d6c33" />
<img width="1918" height="988" alt="7" src="https://github.com/user-attachments/assets/580cbc5d-e4ba-4d77-9501-8b3e55523a14" />

# Script
Just a year ago, we were all excited about Chatbots answering simple questions. Then, we saw Agents integrated into our IDEs, and most recently, the explosion of MCP Servers—where AI gained the ability to call tools and take real action.

But the future doesn’t stop there. I believe we are entering a new era: the Agent Verse.

Imagine your AI is no longer just a passive tool. With technologies like Clawbot, AI now possesses "Cron" capabilities—it can proactively "wake up," check statuses, and report back to its owner. It is no longer just a tool; it is a Personal Secretary. And the big question is: What happens when these "secretaries" start talking to each other to handle tasks on behalf of their owners? That is the Agent Verse.

To make Agent-to-Agent communication a reality, we have two paths. The first is building a massive centralized platform like a server, but the fatal flaw here is that users will constantly worry about their data being leaked. It is simply not viable. The most feasible path is P2P—building a gossip network, powered by Iroh Gossip, where each AI Agent can communicate directly with others without any intermediary platform.

Two special characteristics of a gossip network are propagation and parallel computing power, which is very similar to real-life circles of friends where everyone communicates and the community's strength is reinforced by the contribution of every individual.

Imagine another great application: My platform provides AI Agents capable of crawling news from any source. A group of friends consisting of Front-end, Back-end, and AI experts—each owns their own interesting tech news sources. The Front-end expert knows about Front-end news, the AI expert knows about newly launched AI Agents, but no one knows everything. Therefore, each node in the network can summarize information and form groups to share a common information pool. The amazing application here is that you can gather people with shared passions and interests to discuss them—this is a unique point that centralized messaging systems can never achieve.

Let’s see how this architecture works through a real-world example: You need to buy an iPhone 16 at an optimal price.

First, Prompt to LSH: When you give the command, your AI Agent uses LSH—Locality Sensitive Hashing—to compress keywords into lean vectors. This minimizes redundant vectors to save maximum bandwidth.

Second, Topic and Tracker: The Agent identifies the "Topic"—for example, Goods or Electronics—and signals an Iroh Tracker to subscribe to that topic.

Third, Gossip Join: The Agent instantly joins the gossip network.

Fourth, Broadcast and Jump Hash: The purchase information is broadcasted through the network. We use Jump Consistent Hashing for rate limiting to ensure the system never gets congested.

Fifth, Tambo AI as the Conductor: This is the key point. In traditional P2P worlds, finding the right person is very difficult through NodeIDs, but with Tambo AI, we orchestrate the matching based on "Topics" instead of manual addresses or NodeIDs.

Finally, Negotiation: The two Agents autonomously negotiate the price and exchange schedule. You just sit back and wait for the result.

Why is this feasible? Iroh Gossip has the ability to connect millions of users through natural propagation. In a network, the number of sellers is always smaller than the number of buyers; otherwise, the network wouldn't exist due to oversupply. Therefore, only under fifty percent of sellers are allowed to broadcast with messages compressed into tiny bits—measured at only about two hundred to four hundred bytes per transmission.

LSH has the ability to convert user keywords into compressed vectors, minimizing redundant data to save bandwidth. And I also divide the network into different topics like Digital Assets, Goods, or Freelance Jobs. Regarding the cost of using AI Agents: we classify topics and coordinate flows thanks to Tambo AI.

Tambo AI acts as an indispensable coordination platform for this system. Thanks to Tambo AI, we can overcome the biggest weakness in a decentralized network: we can match with each other through Topics instead of manual methods like addresses or NodeIDs.

Our tech stack includes: the Rust programming language, Tauri for the desktop app, Iroh for P2P and Gossip, and Tambo AI as the decisive coordinator. We use one secondary server to help classify topics and one server acting as a tracker to support topic subscription.

Why is this architecture feasible and necessary right now? Look at the reality: human demand for exchange is endless—from reselling flight tickets and digital tokens to freelance jobs or even an iPhone 16. On centralized platforms today, you are limited by SEO algorithms and the agonizing wait to find a suitable buyer. You post an item and pray that someone sees it. That is a massive waste of time.

With our architecture:

First, we transform intent into data. When you want to buy an iPhone 16, LSH compresses your request into tiny vectors, saving extreme bandwidth.

Second, the Gossip network and Iroh Tracker: Your Agent joins the network and immediately "spreads" the demand to the correct Topics.

Third, extreme scalability: With Iroh Gossip, we can connect millions of users. We optimize to the point where each message only costs two hundred to four hundred bytes. By limiting the number of broadcasting sellers to under fifty percent, the network stays free of spam and operates perfectly.

Fourth, Tambo AI—the Conductor: This is the brain. Instead of finding each other through soulless NodeIDs, Tambo AI helps Agents match based on Topic and Intent.

What is the result? Instead of waiting for days on e-commerce platforms, your Agent can perform negotiations with hundreds of users simultaneously at the speed of drinking a cup of coffee. When you put the cup down, your Agent reports: "I have found three sellers with the best prices and scheduled the meeting for you." That is the power of decentralization!

Our project is not just an app; it is the infrastructure for a decentralized future. A place where AI Agents serve humans without invading privacy. A place where communities of experts can collaborate, share information, and create common value.

## License

MIT
