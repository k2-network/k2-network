import { apiFetch } from './client';

export const initNode = () =>
  apiFetch<{ node_id: string; status: string }>('/api/init', { method: 'POST' });

export const getMyNodeId = () =>
  apiFetch<{ node_id: string }>('/api/node-id').then((r) => r.node_id);

/** Lấy node_id cố định của user đang đăng nhập từ DB (không thay đổi giữa các session) */
export const getUserNodeId = () =>
  apiFetch<{ node_id: string }>('/api/user/node-id').then((r) => r.node_id);
