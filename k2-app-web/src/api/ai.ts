import { apiFetch } from './client';

export interface ClassifyIntentParams {
  user_prompt: string;
  api_key: string;
  base_url?: string;
  model?: string;
}

export const classifyIntent = (params: ClassifyIntentParams) =>
  apiFetch<Record<string, unknown>>('/api/classify-intent', {
    method: 'POST',
    body: JSON.stringify(params),
  });

export interface GroqChatParams {
  messages: unknown;
  tools?: unknown;
  api_key: string;
  model?: string;
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
