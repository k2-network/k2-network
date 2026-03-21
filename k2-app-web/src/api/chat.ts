import { apiFetch } from './client';

export const sendChatMessage = (
  recipient_node_id: string,
  content: string,
  sender_session_id: string,
  sender_name: string,
) =>
  apiFetch<{ status: string }>('/api/chat/send', {
    method: 'POST',
    body: JSON.stringify({ recipient_node_id, content, sender_session_id, sender_name }),
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

/** No-op in relay mode — kept for backwards compatibility */
export const startDmListener = (contact_node_id: string) =>
  apiFetch<{ status: string; contact: string }>('/api/chat/listen', {
    method: 'POST',
    body: JSON.stringify({ contact_node_id }),
  });
