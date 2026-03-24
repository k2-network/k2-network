import { apiFetch } from './client';

export interface Contact {
  node_id: string;
  nickname: string;
  added_at: number;
  notes?: string;
}

export const listContacts = () =>
  apiFetch<Contact[]>('/api/contacts');

export const addContact = (node_id: string, nickname: string, notes?: string) =>
  apiFetch<Contact>('/api/contacts', {
    method: 'POST',
    body: JSON.stringify({ node_id, nickname, notes }),
  });

export const removeContact = (nodeId: string) =>
  apiFetch<{ removed: boolean }>(`/api/contacts/${encodeURIComponent(nodeId)}`, { method: 'DELETE' });

export const updateContactNickname = (nodeId: string, nickname: string) =>
  apiFetch<{ updated: boolean }>(`/api/contacts/${encodeURIComponent(nodeId)}`, {
    method: 'PUT',
    body: JSON.stringify({ nickname }),
  });

export const pingContact = (nodeId: string) =>
  apiFetch<{ online: boolean }>(`/api/contacts/${encodeURIComponent(nodeId)}/ping`, { method: 'POST' })
    .then((r) => r.online);
