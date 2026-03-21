/**
 * AuthGate — màn hình chọn chế độ trước khi vào app
 *
 * 3 bước:
 *   1. ModeSelect  — chọn Guest hoặc Login/Register
 *   2. LoginForm   — đăng nhập
 *   3. RegisterForm — đăng ký
 */
import { useState } from "react";
import k2Logo from "../../assets/k2-logo.svg";
import "./AuthGate.css";

type Screen = "mode-select" | "login" | "register";

interface AuthGateProps {
    onGuest: () => void;
    onAuth: (token: string, nodeId: string, username: string) => void;
}

// ─── Mode Select ────────────────────────────────────────────────────────────

function ModeSelect({ onGuest, onLogin, onRegister }: {
    onGuest: () => void;
    onLogin: () => void;
    onRegister: () => void;
}) {
    return (
        <div className="auth-mode-select">
            <div className="auth-logo">
                <img src={k2Logo} alt="K2" />
            </div>
            <h1 className="auth-title">K2 Network</h1>
            <p className="auth-subtitle">P2P Marketplace — Decentralized Trading</p>

            <div className="auth-cards">
                {/* Guest card */}
                <button className="auth-card auth-card-guest" onClick={onGuest}>
                    <div className="auth-card-icon">
                        <svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor">
                            <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/>
                        </svg>
                    </div>
                    <div className="auth-card-body">
                        <h3>Tiếp tục với tư cách Khách</h3>
                        <p>Trải nghiệm ngay, không cần đăng ký</p>
                    </div>
                    <div className="auth-card-limits">
                        <span className="limit-badge">Tối đa 2 yêu cầu</span>
                        <span className="limit-badge">Không lưu lịch sử</span>
                    </div>
                    <div className="auth-card-arrow">→</div>
                </button>

                {/* Login card */}
                <button className="auth-card auth-card-login" onClick={onLogin}>
                    <div className="auth-card-icon auth-card-icon-primary">
                        <svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor">
                            <path d="M11 7L9.6 8.4l2.6 2.6H2v2h10.2l-2.6 2.6L11 17l5-5-5-5zm9 12h-8v2h8c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2h-8v2h8v14z"/>
                        </svg>
                    </div>
                    <div className="auth-card-body">
                        <h3>Đăng nhập</h3>
                        <p>Dùng tài khoản đã có</p>
                    </div>
                    <div className="auth-card-limits">
                        <span className="limit-badge limit-badge-green">Không giới hạn</span>
                        <span className="limit-badge limit-badge-green">Lưu lịch sử</span>
                        <span className="limit-badge limit-badge-green">Đăng ký Agent</span>
                    </div>
                    <div className="auth-card-arrow">→</div>
                </button>

                {/* Register card */}
                <button className="auth-card auth-card-register" onClick={onRegister}>
                    <div className="auth-card-icon auth-card-icon-accent">
                        <svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor">
                            <path d="M15 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm-9-2V7H4v3H1v2h3v3h2v-3h3v-2H6zm9 4c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/>
                        </svg>
                    </div>
                    <div className="auth-card-body">
                        <h3>Tạo tài khoản</h3>
                        <p>Đăng ký để mở khóa đầy đủ tính năng</p>
                    </div>
                    <div className="auth-card-limits">
                        <span className="limit-badge limit-badge-blue">Node ID cố định</span>
                        <span className="limit-badge limit-badge-blue">Tìm kiếm mở rộng</span>
                    </div>
                    <div className="auth-card-arrow">→</div>
                </button>
            </div>
        </div>
    );
}

// ─── Login Form ──────────────────────────────────────────────────────────────

function LoginForm({ onBack, onSuccess }: {
    onBack: () => void;
    onSuccess: (token: string, nodeId: string, username: string) => void;
}) {
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [error, setError] = useState("");
    const [loading, setLoading] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError("");
        if (!email.trim() || !password) { setError("Vui lòng điền đầy đủ thông tin"); return; }

        setLoading(true);
        try {
            const res = await fetch("/api/auth/login", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ email: email.trim(), password }),
            });
            const data = await res.json();
            if (!res.ok) throw new Error(data.error || "Đăng nhập thất bại");

            localStorage.setItem("k2_refresh_token", data.refresh_token);
            onSuccess(data.access_token, data.node_id, data.username);
        } catch (err) {
            setError(err instanceof Error ? err.message : "Lỗi không xác định");
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="auth-form-container">
            <button className="auth-back-btn" onClick={onBack}>← Quay lại</button>

            <div className="auth-form-header">
                <div className="auth-form-icon">
                    <svg width="28" height="28" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M11 7L9.6 8.4l2.6 2.6H2v2h10.2l-2.6 2.6L11 17l5-5-5-5zm9 12h-8v2h8c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2h-8v2h8v14z"/>
                    </svg>
                </div>
                <h2>Đăng nhập</h2>
                <p>Chào mừng trở lại</p>
            </div>

            <form className="auth-form" onSubmit={handleSubmit}>
                <div className="auth-field">
                    <label>Email</label>
                    <input
                        type="email"
                        placeholder="you@example.com"
                        value={email}
                        onChange={e => setEmail(e.target.value)}
                        autoFocus
                    />
                </div>
                <div className="auth-field">
                    <label>Mật khẩu</label>
                    <input
                        type="password"
                        placeholder="••••••••"
                        value={password}
                        onChange={e => setPassword(e.target.value)}
                    />
                </div>

                {error && <div className="auth-error">{error}</div>}

                <button className="auth-submit-btn" type="submit" disabled={loading}>
                    {loading ? <span className="auth-spinner" /> : "Đăng nhập"}
                </button>
            </form>
        </div>
    );
}

// ─── Register Form ───────────────────────────────────────────────────────────

function RegisterForm({ onBack, onSuccess }: {
    onBack: () => void;
    onSuccess: (token: string, nodeId: string, username: string) => void;
}) {
    const [username, setUsername] = useState("");
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [confirm, setConfirm] = useState("");
    const [error, setError] = useState("");
    const [loading, setLoading] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError("");
        if (!username.trim()) { setError("Tên người dùng không được để trống"); return; }
        if (username.trim().length < 3) { setError("Tên người dùng tối thiểu 3 ký tự"); return; }
        if (!email.trim()) { setError("Email không được để trống"); return; }
        if (password.length < 8) { setError("Mật khẩu tối thiểu 8 ký tự"); return; }
        if (password !== confirm) { setError("Mật khẩu xác nhận không khớp"); return; }

        setLoading(true);
        try {
            const res = await fetch("/api/auth/register", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ username: username.trim(), email: email.trim(), password }),
            });
            const data = await res.json();
            if (!res.ok) throw new Error(data.error || "Đăng ký thất bại");

            localStorage.setItem("k2_refresh_token", data.refresh_token);
            onSuccess(data.access_token, data.node_id, data.username);
        } catch (err) {
            setError(err instanceof Error ? err.message : "Lỗi không xác định");
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="auth-form-container">
            <button className="auth-back-btn" onClick={onBack}>← Quay lại</button>

            <div className="auth-form-header">
                <div className="auth-form-icon auth-form-icon-accent">
                    <svg width="28" height="28" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M15 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm-9-2V7H4v3H1v2h3v3h2v-3h3v-2H6zm9 4c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/>
                    </svg>
                </div>
                <h2>Tạo tài khoản</h2>
                <p>Node ID sẽ được gắn cố định với tài khoản</p>
            </div>

            <form className="auth-form" onSubmit={handleSubmit}>
                <div className="auth-field">
                    <label>Tên người dùng</label>
                    <input
                        type="text"
                        placeholder="anonymous_trader"
                        value={username}
                        onChange={e => setUsername(e.target.value)}
                        autoFocus
                        maxLength={32}
                    />
                </div>
                <div className="auth-field">
                    <label>Email</label>
                    <input
                        type="email"
                        placeholder="you@example.com"
                        value={email}
                        onChange={e => setEmail(e.target.value)}
                    />
                </div>
                <div className="auth-field-row">
                    <div className="auth-field">
                        <label>Mật khẩu</label>
                        <input
                            type="password"
                            placeholder="Tối thiểu 8 ký tự"
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                        />
                    </div>
                    <div className="auth-field">
                        <label>Xác nhận mật khẩu</label>
                        <input
                            type="password"
                            placeholder="••••••••"
                            value={confirm}
                            onChange={e => setConfirm(e.target.value)}
                        />
                    </div>
                </div>

                {error && <div className="auth-error">{error}</div>}

                <button className="auth-submit-btn auth-submit-btn-accent" type="submit" disabled={loading}>
                    {loading ? <span className="auth-spinner" /> : "Tạo tài khoản"}
                </button>
            </form>
        </div>
    );
}

// ─── AuthGate (root) ─────────────────────────────────────────────────────────

export function AuthGate({ onGuest, onAuth }: AuthGateProps) {
    const [screen, setScreen] = useState<Screen>("mode-select");

    return (
        <div className="auth-gate">
            <div className="auth-gate-bg" />
            <div className="auth-gate-content">
                {screen === "mode-select" && (
                    <ModeSelect
                        onGuest={onGuest}
                        onLogin={() => setScreen("login")}
                        onRegister={() => setScreen("register")}
                    />
                )}
                {screen === "login" && (
                    <LoginForm
                        onBack={() => setScreen("mode-select")}
                        onSuccess={onAuth}
                    />
                )}
                {screen === "register" && (
                    <RegisterForm
                        onBack={() => setScreen("mode-select")}
                        onSuccess={onAuth}
                    />
                )}
            </div>
        </div>
    );
}
