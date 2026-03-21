import { apiFetch } from './client';

export const initNode = () =>
  apiFetch<{ node_id: string; status: string }>('/api/init', { method: 'POST' });

export const getMyNodeId = () =>
  apiFetch<{ node_id: string }>('/api/node-id').then((r) => r.node_id);
