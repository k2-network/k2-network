const API_BASE = (import.meta.env.VITE_API_URL as string) ?? '';

export const shareFileBytes = async (file: File): Promise<{ ticket: string; filename: string }> => {
  const form = new FormData();
  form.append('file', file, file.name);
  const res = await fetch(`${API_BASE}/api/files/share`, { method: 'POST', body: form });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`API error ${res.status}: ${text}`);
  }
  return res.json();
};

export const downloadFileUrl = (ticket: string) =>
  `${API_BASE}/api/files/download?ticket=${encodeURIComponent(ticket)}`;
