import { apiFetch } from './client';

export interface FriendRequest {
  id: number;
  from_node_id: string;
  from_username: string;
  status: 'pending' | 'accepted' | 'declined';
  created_at: number;
}

export interface SentRequest {
  id: number;
  to_node_id: string;
  status: 'pending' | 'accepted' | 'declined';
  created_at: number;
}

export const sendFriendRequest = (to_node_id: string) =>
  apiFetch<{ status: string; id: number }>('/api/friend-requests', {
    method: 'POST',
    body: JSON.stringify({ to_node_id }),
  });

export const getPendingRequests = () =>
  apiFetch<FriendRequest[]>('/api/friend-requests/pending');

export const getSentRequests = () =>
  apiFetch<SentRequest[]>('/api/friend-requests/sent');

export const acceptRequest = (id: number) =>
  apiFetch<{ status: string; contact: { node_id: string; nickname: string; added_at: number } }>(
    `/api/friend-requests/${id}/accept`,
    { method: 'PUT' }
  );

export const declineRequest = (id: number) =>
  apiFetch<{ status: string }>(`/api/friend-requests/${id}/decline`, { method: 'PUT' });
