import { useEffect, useState } from "react";
import type { GlassTint, ThemeMode } from "../types/track";

const THEME_STORAGE_KEY = "tagmaster_dj_theme";
const TINT_STORAGE_KEY = "tagmaster_dj_glass_tint";

export function useTheme() {
  const [theme, setTheme] = useState<ThemeMode>(() => {
    const saved = window.localStorage.getItem(THEME_STORAGE_KEY);
    return saved === "light" ? "light" : "dark";
  });

  const [glassTint, setGlassTint] = useState<GlassTint>(() => {
    const saved = window.localStorage.getItem(TINT_STORAGE_KEY);
    return saved === "tinted" ? "tinted" : "clear";
  });

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("dark", "light");
    root.classList.add(theme);
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    document.documentElement.dataset.glassTint = glassTint;
    window.localStorage.setItem(TINT_STORAGE_KEY, glassTint);
  }, [glassTint]);

  const toggleTheme = () => {
    setTheme((prev) => (prev === "dark" ? "light" : "dark"));
  };

  const toggleGlassTint = () => {
    setGlassTint((prev) => (prev === "clear" ? "tinted" : "clear"));
  };

  return {
    theme,
    setTheme,
    toggleTheme,
    glassTint,
    setGlassTint,
    toggleGlassTint,
  };
}
