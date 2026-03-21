import { apiFetch } from './client';

// Mỗi lần load page tạo UUID mới — in-memory, không share giữa các tab
function generateUUID(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

// Tạo 1 lần khi module load — duy nhất cho tab/page này
const SESSION_ID = generateUUID();

export const getMySessionId = () => SESSION_ID;

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
