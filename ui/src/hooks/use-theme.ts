import { useEffect, useEffectEvent, useState } from "react";

export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "cliswitch-theme";

function getSystemTheme(): "light" | "dark" {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const resolved = theme === "system" ? getSystemTheme() : theme;
  root.classList.remove("light", "dark");
  root.classList.add(resolved);
}

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (typeof window === "undefined") return "system";
    return (window.localStorage.getItem(STORAGE_KEY) as Theme) || "system";
  });

  const syncSystemTheme = useEffectEvent(() => {
    if (theme === "system") {
      applyTheme("system");
    }
  });

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", syncSystemTheme);
    return () => media.removeEventListener("change", syncSystemTheme);
  }, [syncSystemTheme]);

  const setTheme = (nextTheme: Theme) => {
    window.localStorage.setItem(STORAGE_KEY, nextTheme);
    setThemeState(nextTheme);
  };

  return { theme, setTheme };
}
