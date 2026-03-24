import { apiFetch } from './client';

export interface ClassifyIntentParams {
  user_prompt: string;
  base_url?: string;
  model?: string;
  session_id?: string;
}

export const classifyIntent = (params: ClassifyIntentParams) =>
  apiFetch<Record<string, unknown>>('/api/classify-intent', {
    method: 'POST',
    body: JSON.stringify(params),
  });

export interface GroqChatParams {
  messages: unknown;
  tools?: unknown;
  model?: string;
  session_id?: string;
}

export const groqChatWithTools = (params: GroqChatParams) =>
  apiFetch<{ content: string; tool_calls: unknown }>('/api/groq-chat', {
    method: 'POST',
    body: JSON.stringify(params),
  });

export const classifyK2Endpoint = (user_prompt: string) =>
  apiFetch<Record<string, unknown>>('/api/k2-endpoint', {
    method: 'POST',
    body: JSON.stringify({ user_prompt }),
  });

export const generateQrSvg = (data: string) =>
  apiFetch<{ svg: string }>('/api/qr-svg', {
    method: 'POST',
    body: JSON.stringify({ data }),
  }).then((r) => r.svg);

/** Lưu API key riêng của user lên server (không trả về lại) */
export const saveUserApiKey = (session_id: string, api_key: string) =>
  apiFetch<{ status: string }>('/api/settings/groq-key', {
    method: 'POST',
    body: JSON.stringify({ session_id, api_key }),
  });

/** Kiểm tra user đã có key riêng chưa (chỉ trả về boolean) */
export const checkUserApiKey = (session_id: string) =>
  apiFetch<{ has_custom_key: boolean; has_default_key: boolean }>(`/api/settings/groq-key?session_id=${session_id}`, {
    method: 'GET',
  });
