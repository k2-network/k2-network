/**
 * AuthContext — quản lý trạng thái đăng nhập toàn app
 *
 * mode:
 *   "loading"  — đang restore session từ refresh token (hiện spinner, không hiện AuthGate)
 *   "pending"  — chưa đăng nhập, cần chọn chế độ (hiện AuthGate)
 *   "guest"    — khách, không đăng nhập
 *   "auth"     — đã đăng nhập
 */
import { createContext, useContext, useEffect, useState, useCallback } from "react";
import { getMySessionId, setMarketplaceSessionId } from "../api/marketplace";
import { setAuthToken, setOnTokenRefreshed } from "../api/client";

export type AuthMode = "loading" | "pending" | "guest" | "auth";

interface AuthUser {
    userId: string;
    username: string;
    nodeId: string;
    accessToken: string;
}

interface AuthContextValue {
    mode: AuthMode;
    user: AuthUser | null;
    /** ID duy nhất cho session này — dùng để route WebSocket messages */
    sessionId: string;
    /** Số trade requests guest đã dùng (0-2) */
    guestRequestCount: number;
    incrementGuestCount: () => void;
    loginAsGuest: () => void;
    loginWithToken: (token: string, nodeId: string, username: string) => void;
    logout: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

const GUEST_COUNT_KEY = "k2_guest_count";
const REFRESH_TOKEN_KEY = "k2_refresh_token";

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [mode, setMode] = useState<AuthMode>("loading");
    const [user, setUser] = useState<AuthUser | null>(null);
    const [guestRequestCount, setGuestRequestCount] = useState(() => {
        return parseInt(localStorage.getItem(GUEST_COUNT_KEY) || "0", 10);
    });

    // sessionId: auth → nodeId (cố định từ DB, unique per-user), guest → UUID ngẫu nhiên
    // nodeId được dùng nhất quán cho WS routing, marketplace session_id, và chat recipient_node_id
    const sessionId = user?.nodeId ?? getMySessionId();

    // Sync token và session_id khi user thay đổi
    useEffect(() => {
        setAuthToken(user?.accessToken ?? null);
        setMarketplaceSessionId(user?.nodeId ?? null);
    }, [user]);

    // Khi apiFetch tự refresh token thành công, cập nhật lại state
    useEffect(() => {
        setOnTokenRefreshed((newToken) => {
            setUser(prev => prev ? { ...prev, accessToken: newToken } : prev);
        });
    }, []);

    // Khi mount: thử restore session từ refresh token
    // mode bắt đầu là "loading" → không hiện AuthGate trong khi đang check
    useEffect(() => {
        const tryRestore = async () => {
            const refreshToken = localStorage.getItem(REFRESH_TOKEN_KEY);
            if (!refreshToken) {
                setMode("pending");
                return;
            }
            try {
                const res = await fetch("/api/auth/refresh", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ refresh_token: refreshToken }),
                });

                if (res.status === 401) {
                    // Token thực sự hết hạn hoặc bị thu hồi → xóa và hiện login
                    localStorage.removeItem(REFRESH_TOKEN_KEY);
                    setMode("pending");
                    return;
                }

                if (!res.ok) {
                    // Lỗi server (5xx), rate limit (429), lỗi mạng tạm thời
                    // Giữ refresh token, thử lại lần sau
                    setMode("pending");
                    return;
                }

                const data = await res.json();
                localStorage.setItem(REFRESH_TOKEN_KEY, data.refresh_token);
                // Set token synchronously trước khi setUser/setMode để tránh race condition
                // với child effects (e.g. ContactPage) chạy trước AuthContext effect
                setAuthToken(data.access_token);
                setMarketplaceSessionId(data.node_id);
                setUser({
                    userId: data.user_id,
                    username: data.username,
                    nodeId: data.node_id,
                    accessToken: data.access_token,
                });
                setMode("auth");
            } catch {
                // Lỗi mạng (fetch throw) — giữ refresh token, không xóa
                setMode("pending");
            }
        };
        tryRestore();
    }, []);

    const loginAsGuest = useCallback(() => {
        setMode("guest");
        setUser(null);
    }, []);

    const loginWithToken = useCallback((token: string, nodeId: string, username: string) => {
        // Set token synchronously trước khi setUser/setMode để tránh race condition
        setAuthToken(token);
        setMarketplaceSessionId(nodeId);
        // decode user_id từ JWT payload (không cần verify ở client)
        try {
            const payload = JSON.parse(atob(token.split(".")[1]));
            setUser({ userId: payload.sub, username, nodeId, accessToken: token });
        } catch {
            setUser({ userId: "", username, nodeId, accessToken: token });
        }
        setMode("auth");
    }, []);

    const logout = useCallback(async () => {
        const refreshToken = localStorage.getItem(REFRESH_TOKEN_KEY);
        if (refreshToken) {
            try {
                await fetch("/api/auth/logout", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ refresh_token: refreshToken }),
                });
            } catch { /* ignore */ }
        }
        localStorage.removeItem(REFRESH_TOKEN_KEY);
        setAuthToken(null);
        setMarketplaceSessionId(null);
        setUser(null);
        setMode("pending");
    }, []);

    const incrementGuestCount = useCallback(() => {
        setGuestRequestCount(prev => {
            const next = prev + 1;
            localStorage.setItem(GUEST_COUNT_KEY, String(next));
            return next;
        });
    }, []);

    return (
        <AuthContext.Provider value={{
            mode,
            user,
            sessionId,
            guestRequestCount,
            incrementGuestCount,
            loginAsGuest,
            loginWithToken,
            logout,
        }}>
            {children}
        </AuthContext.Provider>
    );
}

export function useAuth(): AuthContextValue {
    const ctx = useContext(AuthContext);
    if (!ctx) throw new Error("useAuth must be used inside AuthProvider");
    return ctx;
}
