import { apiFetch } from './client';

export interface TopicPeerEntry {
  node_id: string;
  announced_at: number;
  subtopic?: string;
  action?: string;
}

export interface TopicPeersResponse {
  topic: string;
  peers: TopicPeerEntry[];
  count: number;
}

export const announceTopic = (
  topic: string,
  nodeId: string,
  subtopic?: string,
  action?: string,
): Promise<{ status: string }> =>
  apiFetch('/api/tracker/announce', {
    method: 'POST',
    body: JSON.stringify({
      topic,
      node_id: nodeId,
      ...(subtopic ? { subtopic } : {}),
      ...(action   ? { action }   : {}),
    }),
  });

export const leaveTopic = (topic: string, nodeId: string): Promise<{ status: string }> =>
  apiFetch('/api/tracker/announce', {
    method: 'DELETE',
    body: JSON.stringify({ topic, node_id: nodeId }),
  });

export const getTopicPeers = (topic: string): Promise<TopicPeersResponse> =>
  apiFetch(`/api/tracker/peers?topic=${encodeURIComponent(topic)}`);

// ── Subtopic Stats ───────────────────────────────────────────────────────────

export interface SubtopicStat {
  subtopic: string;
  buy: number;
  sell: number;
  exchange: number;
  unknown: number;
  total: number;
  nodes: {
    buy: string[];
    sell: string[];
    exchange: string[];
    unknown: string[];
  };
}

export interface SubtopicStatsResponse {
  topic: string;
  stats: SubtopicStat[];
}

export const getSubtopicStats = (topic: string): Promise<SubtopicStatsResponse> =>
  apiFetch(`/api/tracker/subtopic-stats?topic=${encodeURIComponent(topic)}`);
