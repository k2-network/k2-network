# K2 Network

K2 is a decentralized P2P marketplace powered by AI agents. Users can buy, sell, and exchange digital assets, goods, and freelance services through an AI-assisted negotiation system without relying on centralized platforms.

## Features

- **AI-Powered Marketplace**: Natural language intent classification for buy/sell/exchange requests
- **P2P Communication**: Direct peer-to-peer connections via Iroh Gossip protocol
- **Contact Management**: P2P-synced contact book with direct messaging
- **File Sharing**: Share and download files directly between peers using iroh-blobs
- **Topic-Based Discovery**: Join marketplace topics to find relevant buyers/sellers
- **Cross-Platform**: Desktop (Windows, macOS, Linux) and Android support

## Installation

### Option 1: Download Installer (Windows)

Download `k2-app-tauri_0.1.0_x64-setup.exe` from the repository and run the installer.

### Option 2: Run from Source

**Prerequisites**
- Rust & Cargo
- Node.js (v18+)
- Tauri CLI: `npm install -g @tauri-apps/cli`
- C++ Build Tools (Windows) or Xcode (macOS)

**Steps**

```bash
cd k2-app-tauri
npm install
npm run tauri dev
```

For Android:
```bash
npm run tauri android dev
```

### Build Production

```bash
npm run tauri build
```

Output: `k2-app-tauri/src-tauri/target/release/bundle/`

## Configuration

Create `.env` file in `k2-app-tauri/`:

```env
VITE_GROQ_API_KEY=your_groq_api_key
VITE_GROQ_BASE_URL=https://api.groq.com/openai/v1
VITE_GROQ_SMALL_MODEL=llama-3.3-70b-versatile
```

## Usage

1. Launch the application
2. Open the AI Chat panel (bottom-right)
3. Describe what you want to buy, sell, or exchange
4. The AI will classify your intent and prepare a dynamic form
5. Submit your request to broadcast on the P2P network

## License

MIT
