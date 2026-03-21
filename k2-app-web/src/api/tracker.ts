import { apiFetch } from './client';

export interface TopicPeerEntry {
  node_id: string;
  announced_at: number;
}

export interface TopicPeersResponse {
  topic: string;
  peers: TopicPeerEntry[];
  count: number;
}

export const announceTopic = (topic: string, nodeId: string): Promise<{ status: string }> =>
  apiFetch('/api/tracker/announce', {
    method: 'POST',
    body: JSON.stringify({ topic, node_id: nodeId }),
  });

export const getTopicPeers = (topic: string): Promise<TopicPeersResponse> =>
  apiFetch(`/api/tracker/peers?topic=${encodeURIComponent(topic)}`);
