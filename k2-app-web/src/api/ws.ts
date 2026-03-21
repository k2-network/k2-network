type K2EventType =
  | 'k2://chat-message'
  | 'k2://offer-received'
  | 'k2://peer-connected'
  | 'k2://peer-disconnected';

// Server sends { "type": "chat_message" | "offer_received" | ... }
// Map to Tauri-style event names
const SERVER_TO_K2: Record<string, K2EventType> = {
  chat_message: 'k2://chat-message',
  offer_received: 'k2://offer-received',
  peer_connected: 'k2://peer-connected',
  peer_disconnected: 'k2://peer-disconnected',
};

type Handler = (payload: unknown) => void;

class K2WebSocketClient {
  private ws: WebSocket | null = null;
  private handlers = new Map<K2EventType, Set<Handler>>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private connected = false;
  private url = '';

  connect(url?: string, sessionId?: string) {
    const WS_URL = (import.meta.env.VITE_WS_URL as string) || '';
    const base = url ?? (WS_URL || `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`);
    this.url = sessionId ? `${base}?session_id=${encodeURIComponent(sessionId)}` : base;
    this._connect();
  }

  private _connect() {
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onerror = null;
      this.ws.onclose = null;
      this.ws.close();
    }
    this.ws = new WebSocket(this.url);

    this.ws.onopen = () => {
      this.connected = true;
      if (this.reconnectTimer) {
        clearTimeout(this.reconnectTimer);
        this.reconnectTimer = null;
      }
    };

    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data as string) as { type: string; payload?: unknown };
        const eventType = SERVER_TO_K2[msg.type];
        if (!eventType) return;

        const handlers = this.handlers.get(eventType);
        if (!handlers) return;

        // For chat_message the server wraps content in .payload; unwrap it
        // For other events fall back to the whole message
        const payload = msg.payload !== undefined ? msg.payload : msg;
        handlers.forEach((h) => h(payload));
      } catch {
        // ignore malformed messages
      }
    };

    this.ws.onerror = () => {
      this.connected = false;
    };

    this.ws.onclose = () => {
      this.connected = false;
      // Reconnect after 3s
      this.reconnectTimer = setTimeout(() => this._connect(), 3000);
    };
  }

  /**
   * Mirror of Tauri's listen() — returns an unlisten function
   */
  listen(event: K2EventType, handler: Handler): () => void {
    if (!this.handlers.has(event)) {
      this.handlers.set(event, new Set());
    }
    this.handlers.get(event)!.add(handler);
    return () => {
      this.handlers.get(event)?.delete(handler);
    };
  }

  /** Reconnect với session_id mới (gọi sau khi có node_id) */
  setSession(sessionId: string) {
    if (!this.url) return;
    // Cập nhật URL với session_id mới rồi reconnect
    const base = this.url.split('?')[0];
    this.url = `${base}?session_id=${encodeURIComponent(sessionId)}`;
    this._connect();
  }

  isConnected() {
    return this.connected;
  }
}

export const k2ws = new K2WebSocketClient();
