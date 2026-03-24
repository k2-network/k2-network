import { apiFetch } from './client';

// ── Chat history ──────────────────────────────────────────────────────────────

export interface SaveMessageItem {
  role: 'user' | 'assistant';
  content: string;
  sender_name?: string;
}

export interface ChatHistoryMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  sender_name?: string;
  reply_to_content?: string;
  reply_to_sender?: string;
  created_at: number;
}

/** Save one or more messages to the chat history (AI or P2P) */
export const saveChatMessages = (
  session_id: string,
  conversation_id: string,
  messages: SaveMessageItem[],
) =>
  apiFetch<{ status: string; count: number }>('/api/chat/messages', {
    method: 'POST',
    body: JSON.stringify({ session_id, conversation_id, messages }),
  });

/** Get chat history for a session (returns oldest-first) */
export const getChatHistory = (
  session_id: string,
  conversation_id: string = 'ai',
  limit: number = 50,
) =>
  apiFetch<{ messages: ChatHistoryMessage[] }>(
    `/api/chat/history?session_id=${encodeURIComponent(session_id)}&conversation_id=${encodeURIComponent(conversation_id)}&limit=${limit}`,
  );

// ── P2P messaging ─────────────────────────────────────────────────────────────

export const sendChatMessage = (
  recipient_node_id: string,
  content: string,
  sender_session_id: string,
  sender_name: string,
  sender_node_id: string,
  reply_to_content?: string,
  reply_to_sender?: string,
) =>
  apiFetch<{ status: string }>('/api/chat/send', {
    method: 'POST',
    body: JSON.stringify({ recipient_node_id, content, sender_session_id, sender_name, sender_node_id, reply_to_content, reply_to_sender }),
  });

/** Send a direct P2P message via Iroh to the recipient's K2Node */
export const sendP2pMessage = (
  recipient_node_id: string,
  content: string,
  sender_session_id: string,
  sender_name: string,
) =>
  apiFetch<{ status: string }>('/api/chat/send-p2p', {
    method: 'POST',
    body: JSON.stringify({ recipient_node_id, content, sender_session_id, sender_name }),
  });

export interface UploadedFile {
  file_id: string;
  url: string;
  filename: string;
  size: number;
  mime_type: string;
}

/** Upload file/ảnh vào chat (tối đa 50MB) */
export const uploadChatFile = async (file: File): Promise<UploadedFile> => {
  const API_BASE = (import.meta.env.VITE_API_URL as string) ?? '';
  const form = new FormData();
  form.append('file', file, file.name);
  const res = await fetch(`${API_BASE}/api/chat/upload`, { method: 'POST', body: form });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Upload failed ${res.status}: ${text}`);
  }
  return res.json();
};

/** Xóa toàn bộ tin nhắn của một conversation phía người gọi */
export const clearChatHistory = (
  session_id: string,
  conversation_id: string,
) =>
  apiFetch<{ status: string; deleted: number }>(
    `/api/chat/history?session_id=${encodeURIComponent(session_id)}&conversation_id=${encodeURIComponent(conversation_id)}`,
    { method: 'DELETE' },
  );

/** No-op in relay mode — kept for backwards compatibility */
export const startDmListener = (contact_node_id: string) =>
  apiFetch<{ status: string; contact: string }>('/api/chat/listen', {
    method: 'POST',
    body: JSON.stringify({ contact_node_id }),
  });
