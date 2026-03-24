const API_BASE = (import.meta.env.VITE_API_URL as string) ?? '';

let _authToken: string | null = null;
// Callback để AuthContext cập nhật token mới sau khi refresh
let _onTokenRefreshed: ((token: string) => void) | null = null;
let _refreshing: Promise<string | null> | null = null;

export function setAuthToken(token: string | null) {
  _authToken = token;
}

export function setOnTokenRefreshed(cb: (token: string) => void) {
  _onTokenRefreshed = cb;
}

async function tryRefreshToken(): Promise<string | null> {
  // Chỉ refresh 1 lần nếu có nhiều request song song cùng nhận 401
  if (_refreshing) return _refreshing;

  _refreshing = (async () => {
    const refreshToken = localStorage.getItem('k2_refresh_token');
    if (!refreshToken) return null;
    try {
      const res = await fetch('/api/auth/refresh', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: refreshToken }),
      });
      if (!res.ok) {
        localStorage.removeItem('k2_refresh_token');
        return null;
      }
      const data = await res.json();
      localStorage.setItem('k2_refresh_token', data.refresh_token);
      _authToken = data.access_token;
      _onTokenRefreshed?.(data.access_token);
      return data.access_token;
    } catch {
      return null;
    } finally {
      _refreshing = null;
    }
  })();

  return _refreshing;
}

export async function apiFetch<T>(
  path: string,
  options?: RequestInit
): Promise<T> {
  const authHeader = (): Record<string, string> =>
    _authToken ? { Authorization: `Bearer ${_authToken}` } : {};

  const res = await fetch(`${API_BASE}${path}`, {
    headers: { 'Content-Type': 'application/json', ...authHeader(), ...options?.headers },
    ...options,
  });

  // Token hết hạn → thử refresh rồi retry 1 lần
  if (res.status === 401 && _authToken) {
    const newToken = await tryRefreshToken();
    if (newToken) {
      const retryRes = await fetch(`${API_BASE}${path}`, {
        headers: { 'Content-Type': 'application/json', ...authHeader(), ...options?.headers },
        ...options,
      });
      if (!retryRes.ok) {
        const text = await retryRes.text();
        throw new Error(`API error ${retryRes.status}: ${text}`);
      }
      return retryRes.json() as Promise<T>;
    }
  }

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`API error ${res.status}: ${text}`);
  }
  return res.json() as Promise<T>;
}
