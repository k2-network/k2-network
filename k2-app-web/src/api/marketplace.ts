import { apiFetch } from './client';

function generateUUID(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

// Guest session ID: persist in localStorage so it survives page refreshes
const GUEST_SESSION_ID_KEY = 'k2_guest_session_id';
function getOrCreateGuestSessionId(): string {
  let id = localStorage.getItem(GUEST_SESSION_ID_KEY);
  if (!id) {
    id = generateUUID();
    localStorage.setItem(GUEST_SESSION_ID_KEY, id);
  }
  return id;
}
const GUEST_SESSION_ID = getOrCreateGuestSessionId();

// Override bởi AuthContext khi user đăng nhập — đảm bảo session_id nhất quán với WS routing
let _overrideSessionId: string | null = null;

export const setMarketplaceSessionId = (id: string | null) => { _overrideSessionId = id; };
export const getMySessionId = () => _overrideSessionId ?? GUEST_SESSION_ID;

export const getBroadcastDelay = () =>
  apiFetch<{ delay: number }>('/api/broadcast-delay').then((r) => r.delay);

export const joinTopic = (topic: string, action: string) =>
  apiFetch<{ status: string; topic: string }>('/api/topics/join', {
    method: 'POST',
    body: JSON.stringify({ topic, action }),
  });

export const broadcastOffer = (topic: string, form_data: unknown) =>
  apiFetch<{ status: string; offer_id: string }>('/api/topics/broadcast', {
    method: 'POST',
    body: JSON.stringify({ topic, form_data, session_id: getMySessionId() }),
  });

export const sendInterest = (topic: string, seller_node_id: string, form_data: unknown) =>
  apiFetch<{ status: string; target: string }>('/api/topics/interest', {
    method: 'POST',
    body: JSON.stringify({ topic, seller_node_id, form_data, session_id: getMySessionId() }),
  });

export const listenOffers = (topic: string, timeout?: number) => {
  const params = new URLSearchParams({ topic });
  if (timeout != null) params.set('timeout', String(timeout));
  return apiFetch<unknown[]>(`/api/topics/offers?${params}`);
};

export const startListening = (topic: string) =>
  apiFetch<{ status: string; topic: string }>('/api/topics/listen', {
    method: 'POST',
    body: JSON.stringify({ topic }),
  });

// ── Web Matching Engine ────────────────────────────────────────────────────────

export const postOffer = (topic: string, action: string, form_data: unknown) =>
  apiFetch<{ status: string; offer_id: string; match?: unknown }>('/api/offers', {
    method: 'POST',
    body: JSON.stringify({ topic, action, form_data, session_id: getMySessionId() }),
  });

export const getOffers = (topic?: string) => {
  const params = topic ? `?topic=${encodeURIComponent(topic)}` : '';
  return apiFetch<unknown[]>(`/api/offers${params}`);
};

export const cancelOffer = (topic: string) =>
  apiFetch<{ status: string }>('/api/offers', {
    method: 'DELETE',
    body: JSON.stringify({ session_id: getMySessionId(), topic }),
  });
