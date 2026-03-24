import { useRef, useState, useEffect } from "react";
import { useTheme } from "../../context/ThemeContext";
import "./ThemeSwitcher.css";

export function ThemeSwitcher() {
  const { theme, setTheme, themes } = useTheme();
  const [open, setOpen] = useState(false);
  const [dropPos, setDropPos] = useState<{ top: number; right: number } | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const btnRef = useRef<HTMLButtonElement>(null);

  const current = themes.find((t) => t.value === theme)!;

  // Close on outside click
  useEffect(() => {
    function handler(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const handleToggle = () => {
    if (!open && btnRef.current) {
      const rect = btnRef.current.getBoundingClientRect();
      setDropPos({ top: rect.bottom + 8, right: window.innerWidth - rect.right });
    }
    setOpen((o) => !o);
  };

  return (
    <div className="theme-switcher" ref={ref}>
      <button
        ref={btnRef}
        className="theme-switcher-btn"
        onClick={handleToggle}
        aria-label="Switch theme"
        title="Switch theme"
      >
        <span
          className="theme-swatch"
          style={{ background: current.swatch }}
        />
        <span className="theme-label">{current.label}</span>
        <svg
          className={`theme-chevron ${open ? "open" : ""}`}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      {open && dropPos && (
        <div className="theme-dropdown" style={{ position: 'fixed', top: dropPos.top, right: dropPos.right }}>
          {themes.map((t) => (
            <button
              key={t.value}
              className={`theme-option ${t.value === theme ? "active" : ""}`}
              onClick={() => {
                setTheme(t.value);
                setOpen(false);
              }}
            >
              <span className="theme-swatch" style={{ background: t.swatch }} />
              <span>{t.label}</span>
              {t.value === theme && (
                <svg
                  className="theme-check"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
