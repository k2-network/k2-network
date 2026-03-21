/**
 * AuthContext — quản lý trạng thái đăng nhập toàn app
 *
 * mode:
 *   "pending"  — chưa biết (đang check token cũ trong localStorage)
 *   "guest"    — khách, không đăng nhập
 *   "auth"     — đã đăng nhập
 */
import { createContext, useContext, useEffect, useState, useCallback } from "react";
import { getMySessionId } from "../api/marketplace";

export type AuthMode = "pending" | "guest" | "auth";

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
    const [mode, setMode] = useState<AuthMode>("pending");
    const [user, setUser] = useState<AuthUser | null>(null);
    const [guestRequestCount, setGuestRequestCount] = useState(() => {
        return parseInt(localStorage.getItem(GUEST_COUNT_KEY) || "0", 10);
    });

    // sessionId: auth → userId, guest → UUID từ sessionStorage (cùng nguồn với marketplace)
    const sessionId = user?.userId ?? getMySessionId();

    // Khi mount: thử restore session từ refresh token
    useEffect(() => {
        const tryRestore = async () => {
            const refreshToken = localStorage.getItem(REFRESH_TOKEN_KEY);
            if (!refreshToken) {
                setMode("pending"); // hiện AuthGate
                return;
            }
            try {
                const res = await fetch("/api/auth/refresh", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ refresh_token: refreshToken }),
                });
                if (!res.ok) throw new Error("Token expired");
                const data = await res.json();
                setUser({
                    userId: data.user_id,
                    username: data.username,
                    nodeId: data.node_id,
                    accessToken: data.access_token,
                });
                setMode("auth");
            } catch {
                localStorage.removeItem(REFRESH_TOKEN_KEY);
                setMode("pending"); // hiện AuthGate
            }
        };
        tryRestore();
    }, []);

    const loginAsGuest = useCallback(() => {
        setMode("guest");
        setUser(null);
    }, []);

    const loginWithToken = useCallback((token: string, nodeId: string, username: string) => {
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
