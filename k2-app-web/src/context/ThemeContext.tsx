import { createContext, useContext, useEffect, useState } from "react";

export type Theme = "dark" | "light" | "ocean" | "sunset" | "mint";

const THEMES: { value: Theme; label: string; swatch: string }[] = [
  { value: "dark",   label: "Dark",   swatch: "#7C5CFF" },
  { value: "light",  label: "Light",  swatch: "#4F46E5" },
  { value: "ocean",  label: "Ocean",  swatch: "#3B82F6" },
  { value: "sunset", label: "Sunset", swatch: "#FF4D6D" },
  { value: "mint",   label: "Mint",   swatch: "#34D399" },
];

interface ThemeContextValue {
  theme: Theme;
  setTheme: (t: Theme) => void;
  themes: typeof THEMES;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    return (localStorage.getItem("k2_theme") as Theme) ?? "ocean";
  });

  const setTheme = (t: Theme) => {
    setThemeState(t);
    localStorage.setItem("k2_theme", t);
    document.documentElement.setAttribute("data-theme", t);
  };

  // Apply on mount
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, []);

  return (
    <ThemeContext.Provider value={{ theme, setTheme, themes: THEMES }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used inside ThemeProvider");
  return ctx;
}
