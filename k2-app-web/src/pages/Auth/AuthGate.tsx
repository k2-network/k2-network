/**
 * AuthGate — Premium K2-Team authentication page
 *
 * Split-screen layout with animated network visualization (left)
 * and glassmorphism login/register form (right).
 */
import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import k2Logo from "../../assets/k2-logo.svg";
import { ThemeSwitcher } from "../../components/ThemeSwitcher/ThemeSwitcher";
import "./AuthGate.css";

type Tab = "login" | "register";

interface AuthGateProps {
  onGuest: () => void;
  onAuth: (token: string, nodeId: string, username: string) => void;
}

// ─── Particle Network Canvas ───────────────────────────────────────────────

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  opacity: number;
}

function ParticleNetwork() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const particles = useRef<Particle[]>([]);
  const animRef = useRef<number>(0);
  const mouse = useRef({ x: -1000, y: -1000 });

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let cw = 0;
    let ch = 0;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      cw = rect.width;
      ch = rect.height;
      canvas.width = cw * dpr;
      canvas.height = ch * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    window.addEventListener("resize", resize);

    // Initialize particles spread across entire canvas
    const count = 80;
    particles.current = Array.from({ length: count }, () => ({
      x: Math.random() * cw,
      y: Math.random() * ch,
      vx: (Math.random() - 0.5) * 0.5,
      vy: (Math.random() - 0.5) * 0.5,
      radius: Math.random() * 1.8 + 0.5,
      opacity: Math.random() * 0.5 + 0.2,
    }));

    const handleMouseMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      mouse.current = { x: e.clientX - rect.left, y: e.clientY - rect.top };
    };
    canvas.addEventListener("mousemove", handleMouseMove);

    const animate = () => {
      ctx.clearRect(0, 0, cw, ch);

      const pts = particles.current;
      for (const p of pts) {
        p.x += p.vx;
        p.y += p.vy;
        if (p.x < 0 || p.x > cw) p.vx *= -1;
        if (p.y < 0 || p.y > ch) p.vy *= -1;

        // Mouse attraction
        const dx = mouse.current.x - p.x;
        const dy = mouse.current.y - p.y;
        const dist = Math.sqrt(dx * dx + dy * dy);
        if (dist < 200) {
          p.vx += dx * 0.00008;
          p.vy += dy * 0.00008;
        }
      }

      // Draw connections
      const maxDist = 140;
      for (let i = 0; i < pts.length; i++) {
        for (let j = i + 1; j < pts.length; j++) {
          const dx = pts[i].x - pts[j].x;
          const dy = pts[i].y - pts[j].y;
          const d = Math.sqrt(dx * dx + dy * dy);
          if (d < maxDist) {
            const alpha = (1 - d / maxDist) * 0.15;
            ctx.beginPath();
            ctx.moveTo(pts[i].x, pts[i].y);
            ctx.lineTo(pts[j].x, pts[j].y);
            ctx.strokeStyle = `rgba(139, 92, 246, ${alpha})`;
            ctx.lineWidth = 0.6;
            ctx.stroke();
          }
        }
      }

      // Draw particles
      for (const p of pts) {
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.radius, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(167, 139, 250, ${p.opacity})`;
        ctx.fill();
      }

      animRef.current = requestAnimationFrame(animate);
    };
    animate();

    return () => {
      cancelAnimationFrame(animRef.current);
      window.removeEventListener("resize", resize);
      canvas.removeEventListener("mousemove", handleMouseMove);
    };
  }, []);

  return <canvas ref={canvasRef} className="auth-left-canvas" />;
}

// ─── Password Strength ─────────────────────────────────────────────────────

function getPasswordStrength(pw: string): { level: number; label: string; cls: string } {
  if (!pw) return { level: 0, label: "", cls: "" };
  let score = 0;
  if (pw.length >= 8) score++;
  if (pw.length >= 12) score++;
  if (/[a-z]/.test(pw) && /[A-Z]/.test(pw)) score++;
  if (/\d/.test(pw)) score++;
  if (/[^a-zA-Z0-9]/.test(pw)) score++;

  if (score <= 1) return { level: 1, label: "Weak", cls: "s-weak" };
  if (score <= 2) return { level: 2, label: "Fair", cls: "s-fair" };
  if (score <= 3) return { level: 3, label: "Good", cls: "s-good" };
  return { level: 4, label: "Strong", cls: "s-strong" };
}

function PasswordStrength({ password }: { password: string }) {
  const strength = getPasswordStrength(password);
  if (!password) return null;

  return (
    <div>
      <div className="auth-strength">
        {[1, 2, 3, 4].map((i) => (
          <div key={i} className="auth-strength-bar">
            <div className={`auth-strength-bar-fill ${i <= strength.level ? strength.cls : ""}`} />
          </div>
        ))}
      </div>
      <div className={`auth-strength-label ${strength.cls}`}>{strength.label}</div>
    </div>
  );
}

// ─── Social Icons ──────────────────────────────────────────────────────────

function GoogleIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none">
      <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z" fill="#4285F4"/>
      <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/>
      <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05"/>
      <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/>
    </svg>
  );
}

function GitHubIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor">
      <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"/>
    </svg>
  );
}

function AppleIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor">
      <path d="M17.05 20.28c-.98.95-2.05.88-3.08.4-1.09-.5-2.08-.48-3.24 0-1.44.62-2.2.44-3.06-.4C2.79 15.25 3.51 7.59 9.05 7.31c1.35.07 2.29.74 3.08.8 1.18-.24 2.31-.93 3.57-.84 1.51.12 2.65.72 3.4 1.8-3.12 1.87-2.38 5.98.48 7.13-.57 1.5-1.31 2.99-2.54 4.09zM12.03 7.25c-.15-2.23 1.66-4.07 3.74-4.25.29 2.58-2.34 4.5-3.74 4.25z"/>
    </svg>
  );
}

// ─── Eye Icons ─────────────────────────────────────────────────────────────

function EyeIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
      <circle cx="12" cy="12" r="3"/>
    </svg>
  );
}

function EyeOffIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19m-6.72-1.07a3 3 0 11-4.24-4.24"/>
      <line x1="1" y1="1" x2="23" y2="23"/>
    </svg>
  );
}

// ─── Ripple Button Handler ─────────────────────────────────────────────────

function useRipple() {
  const [ripples, setRipples] = useState<{ x: number; y: number; id: number }[]>([]);

  const addRipple = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const id = Date.now();
    setRipples((prev) => [...prev, { x, y, id }]);
    setTimeout(() => setRipples((prev) => prev.filter((r) => r.id !== id)), 600);
  }, []);

  return { ripples, addRipple };
}

// ─── Floating Label Input ──────────────────────────────────────────────────

function FloatingInput({
  label,
  type = "text",
  value,
  onChange,
  autoFocus,
  maxLength,
  error,
  success,
  hint,
  showToggle,
}: {
  label: string;
  type?: string;
  value: string;
  onChange: (v: string) => void;
  autoFocus?: boolean;
  maxLength?: number;
  error?: boolean;
  success?: boolean;
  hint?: string;
  showToggle?: boolean;
}) {
  const [showPw, setShowPw] = useState(false);
  const inputType = showToggle ? (showPw ? "text" : "password") : type;

  const fieldClass = [
    "auth-field",
    error ? "auth-field-error" : "",
    success ? "auth-field-success" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={fieldClass}>
      <input
        type={inputType}
        placeholder=" "
        value={value}
        onChange={(e) => onChange(e.target.value)}
        autoFocus={autoFocus}
        maxLength={maxLength}
      />
      <label>{label}</label>
      {showToggle && value && (
        <button
          type="button"
          className="auth-password-toggle"
          onClick={() => setShowPw(!showPw)}
          tabIndex={-1}
          aria-label={showPw ? "Hide password" : "Show password"}
        >
          {showPw ? <EyeOffIcon /> : <EyeIcon />}
        </button>
      )}
      {hint && (
        <div className={`auth-field-hint ${error ? "auth-field-hint-error" : "auth-field-hint-success"}`}>
          {hint}
        </div>
      )}
    </div>
  );
}

// ─── Form Animations ───────────────────────────────────────────────────────

const formVariants = {
  enter: { opacity: 0, x: 20, filter: "blur(4px)" },
  center: { opacity: 1, x: 0, filter: "blur(0px)" },
  exit: { opacity: 0, x: -20, filter: "blur(4px)" },
};

// ─── Login Form ────────────────────────────────────────────────────────────

function LoginForm({
  onSuccess,
  onGuest,
}: {
  onSuccess: (token: string, nodeId: string, username: string) => void;
  onGuest: () => void;
}) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const { ripples, addRipple } = useRipple();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!email.trim() || !password) {
      setError("Please fill in all fields");
      return;
    }

    setLoading(true);
    try {
      const res = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email: email.trim(), password }),
      });
      if (res.status === 429) throw new Error("Too many attempts. Please wait a moment and try again.");
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Login failed");

      localStorage.setItem("k2_refresh_token", data.refresh_token);
      if (remember) localStorage.setItem("k2_remember_email", email.trim());
      else localStorage.removeItem("k2_remember_email");
      onSuccess(data.access_token, data.node_id, data.username);
    } catch (err) {
      setError(err instanceof Error ? err.message : "An unexpected error occurred");
    } finally {
      setLoading(false);
    }
  };

  // Restore remembered email
  useEffect(() => {
    const saved = localStorage.getItem("k2_remember_email");
    if (saved) {
      setEmail(saved);
      setRemember(true);
    }
  }, []);

  return (
    <motion.div
      key="login"
      variants={formVariants}
      initial="enter"
      animate="center"
      exit="exit"
      transition={{ duration: 0.35, ease: [0.4, 0, 0.2, 1] }}
    >
      <div className="auth-form-header">
        <h2>Welcome back</h2>
        <p>Sign in to your K2-Team account</p>
      </div>

      <form className="auth-form" onSubmit={handleSubmit}>
        <FloatingInput
          label="Email address"
          type="email"
          value={email}
          onChange={setEmail}
          autoFocus
        />
        <FloatingInput
          label="Password"
          type="password"
          value={password}
          onChange={setPassword}
          showToggle
        />

        <div className="auth-extras">
          <label className="auth-remember">
            <input
              type="checkbox"
              checked={remember}
              onChange={(e) => setRemember(e.target.checked)}
            />
            <span>Remember me</span>
          </label>
          <button type="button" className="auth-forgot">
            Forgot password?
          </button>
        </div>

        <AnimatePresence mode="wait">
          {error && (
            <motion.div
              className="auth-error"
              initial={{ opacity: 0, y: -8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.2 }}
            >
              <svg viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
              </svg>
              {error}
            </motion.div>
          )}
        </AnimatePresence>

        <button
          className="auth-submit-btn"
          type="submit"
          disabled={loading}
          onClick={addRipple}
        >
          {ripples.map((r) => (
            <span key={r.id} className="ripple" style={{ left: r.x, top: r.y }} />
          ))}
          {loading ? <span className="auth-spinner" /> : "Sign In"}
        </button>

        <div className="auth-divider">
          <span>or continue with</span>
        </div>

        <div className="auth-social-buttons">
          <button type="button" className="auth-social-btn">
            <GoogleIcon />
            Google
          </button>
          <button type="button" className="auth-social-btn">
            <GitHubIcon />
            GitHub
          </button>
          <button type="button" className="auth-social-btn">
            <AppleIcon />
            Apple
          </button>
        </div>

        <div className="auth-guest-link">
          <button type="button" onClick={onGuest}>
            Or <span>continue as guest</span> (limited to 2 requests)
          </button>
        </div>
      </form>
    </motion.div>
  );
}

// ─── Register Form ─────────────────────────────────────────────────────────

function RegisterForm({
  onSuccess,
  onGuest,
}: {
  onSuccess: (token: string, nodeId: string, username: string) => void;
  onGuest: () => void;
}) {
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const { ripples, addRipple } = useRipple();

  const emailValid = useMemo(() => {
    if (!email) return null;
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
  }, [email]);

  const passwordsMatch = useMemo(() => {
    if (!confirm) return null;
    return password === confirm;
  }, [password, confirm]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!username.trim()) { setError("Username is required"); return; }
    if (username.trim().length < 3) { setError("Username must be at least 3 characters"); return; }
    if (!email.trim()) { setError("Email is required"); return; }
    if (emailValid === false) { setError("Please enter a valid email"); return; }
    if (password.length < 8) { setError("Password must be at least 8 characters"); return; }
    if (password !== confirm) { setError("Passwords do not match"); return; }

    setLoading(true);
    try {
      const res = await fetch("/api/auth/register", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username: username.trim(), email: email.trim(), password }),
      });
      if (res.status === 429) throw new Error("Too many attempts. Please wait a moment and try again.");
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Registration failed");

      localStorage.setItem("k2_refresh_token", data.refresh_token);
      onSuccess(data.access_token, data.node_id, data.username);
    } catch (err) {
      setError(err instanceof Error ? err.message : "An unexpected error occurred");
    } finally {
      setLoading(false);
    }
  };

  return (
    <motion.div
      key="register"
      variants={formVariants}
      initial="enter"
      animate="center"
      exit="exit"
      transition={{ duration: 0.35, ease: [0.4, 0, 0.2, 1] }}
    >
      <div className="auth-form-header">
        <h2>Create your account</h2>
        <p>Start building on K2-Team today</p>
      </div>

      <form className="auth-form" onSubmit={handleSubmit}>
        <FloatingInput
          label="Username"
          value={username}
          onChange={setUsername}
          autoFocus
          maxLength={32}
          error={username.length > 0 && username.length < 3}
          hint={username.length > 0 && username.length < 3 ? "Min 3 characters" : undefined}
        />
        <FloatingInput
          label="Email address"
          type="email"
          value={email}
          onChange={setEmail}
          error={emailValid === false}
          success={emailValid === true}
          hint={emailValid === false ? "Enter a valid email" : undefined}
        />
        <FloatingInput
          label="Password"
          type="password"
          value={password}
          onChange={setPassword}
          showToggle
        />
        {password && <PasswordStrength password={password} />}
        <FloatingInput
          label="Confirm password"
          type="password"
          value={confirm}
          onChange={setConfirm}
          showToggle
          error={passwordsMatch === false}
          success={passwordsMatch === true}
          hint={passwordsMatch === false ? "Passwords do not match" : undefined}
        />

        <AnimatePresence mode="wait">
          {error && (
            <motion.div
              className="auth-error"
              initial={{ opacity: 0, y: -8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.2 }}
            >
              <svg viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
              </svg>
              {error}
            </motion.div>
          )}
        </AnimatePresence>

        <button
          className="auth-submit-btn"
          type="submit"
          disabled={loading}
          onClick={addRipple}
        >
          {ripples.map((r) => (
            <span key={r.id} className="ripple" style={{ left: r.x, top: r.y }} />
          ))}
          {loading ? <span className="auth-spinner" /> : "Create Account"}
        </button>

        <div className="auth-divider">
          <span>or continue with</span>
        </div>

        <div className="auth-social-buttons">
          <button type="button" className="auth-social-btn">
            <GoogleIcon />
            Google
          </button>
          <button type="button" className="auth-social-btn">
            <GitHubIcon />
            GitHub
          </button>
          <button type="button" className="auth-social-btn">
            <AppleIcon />
            Apple
          </button>
        </div>

        <div className="auth-guest-link">
          <button type="button" onClick={onGuest}>
            Or <span>continue as guest</span> (limited to 2 requests)
          </button>
        </div>
      </form>
    </motion.div>
  );
}

// ─── AuthGate (root) ───────────────────────────────────────────────────────

export function AuthGate({ onGuest, onAuth }: AuthGateProps) {
  const [tab, setTab] = useState<Tab>("login");
  const tabsRef = useRef<HTMLDivElement>(null);
  const [indicatorStyle, setIndicatorStyle] = useState({ left: 0, width: 0 });

  // Update tab indicator position
  useEffect(() => {
    const container = tabsRef.current;
    if (!container) return;
    const buttons = container.querySelectorAll<HTMLButtonElement>(".auth-tab");
    const idx = tab === "login" ? 0 : 1;
    const btn = buttons[idx];
    if (btn) {
      setIndicatorStyle({
        left: btn.offsetLeft,
        width: btn.offsetWidth,
      });
    }
  }, [tab]);

  return (
    <div className="auth-gate">
      {/* Top navbar */}
      <nav className="auth-navbar">
        <div className="auth-navbar-logo">
          <div className="auth-navbar-logo-icon">
            <img src={k2Logo} alt="K2-Team logo" style={{ width: "100%", height: "100%", objectFit: "contain" }} />
          </div>
          <span className="auth-navbar-logo-text">K2-Team</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          <ThemeSwitcher />
          <button className="auth-navbar-back">Back to Home</button>
        </div>
      </nav>

      {/* Split layout */}
      <div className="auth-split">
        {/* Left — visual panel */}
        <div className="auth-left">
          <ParticleNetwork />
          <motion.div
            className="auth-left-content"
            initial={{ opacity: 0, y: 30 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.8, delay: 0.2, ease: [0.4, 0, 0.2, 1] }}
          >
            <div className="auth-left-glow">
              <img src={k2Logo} alt="K2-Team" />
            </div>
            <h1 className="auth-tagline">
              Powering Intelligent
              <br />
              Connections
            </h1>
            <p className="auth-tagline-sub">
              Decentralized AI marketplace for the next generation of digital trade.
              <br />
              Secure. Intelligent. Unstoppable.
            </p>
            <div className="auth-trust-badges">
              <div className="auth-trust-badge">
                <div className="auth-trust-badge-value">P2P</div>
                <div className="auth-trust-badge-label">Architecture</div>
              </div>
              <div className="auth-trust-badge">
                <div className="auth-trust-badge-value">E2E</div>
                <div className="auth-trust-badge-label">Encrypted</div>
              </div>
              <div className="auth-trust-badge">
                <div className="auth-trust-badge-value">AI</div>
                <div className="auth-trust-badge-label">Powered</div>
              </div>
            </div>
          </motion.div>
        </div>

        {/* Right — form panel */}
        <div className="auth-right">
          <motion.div
            className="auth-glass-card"
            initial={{ opacity: 0, scale: 0.96, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            transition={{ duration: 0.6, delay: 0.1, ease: [0.4, 0, 0.2, 1] }}
          >
            {/* Tab toggle */}
            <div className="auth-tabs" ref={tabsRef}>
              <div
                className="auth-tab-indicator"
                style={{ left: indicatorStyle.left, width: indicatorStyle.width }}
              />
              <button
                type="button"
                className={`auth-tab ${tab === "login" ? "auth-tab-active" : ""}`}
                onClick={() => setTab("login")}
              >
                Sign In
              </button>
              <button
                type="button"
                className={`auth-tab ${tab === "register" ? "auth-tab-active" : ""}`}
                onClick={() => setTab("register")}
              >
                Sign Up
              </button>
            </div>

            {/* Animated form switch */}
            <AnimatePresence mode="wait">
              {tab === "login" ? (
                <LoginForm key="login" onSuccess={onAuth} onGuest={onGuest} />
              ) : (
                <RegisterForm key="register" onSuccess={onAuth} onGuest={onGuest} />
              )}
            </AnimatePresence>
          </motion.div>
        </div>
      </div>
    </div>
  );
}
