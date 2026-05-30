import { invoke } from "@tauri-apps/api/core";

const STORAGE_KEY = "deep-cuts-theme";

function createThemeStore() {
  let currentTheme = $state("system");
  let resolvedTheme = $state("dark");

  function applyTheme(theme: string) {
    if (theme === "system") {
      const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      resolvedTheme = isDark ? "dark" : "light";
    } else {
      resolvedTheme = theme;
    }
    document.documentElement.setAttribute("data-theme", resolvedTheme);
  }

  async function setTheme(theme: string, saveToDb = true, tauriConnected = true) {
    currentTheme = theme;
    localStorage.setItem(STORAGE_KEY, theme);
    applyTheme(theme);

    if (saveToDb && tauriConnected) {
      try {
        await invoke("save_theme", { theme });
      } catch (e) {
        console.error("Failed to save theme in Tauri database:", e);
      }
    }
  }

  // System preference listener — active only while currentTheme === "system"
  function initSystemListener() {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      if (currentTheme !== "system") return;
      resolvedTheme = e.matches ? "dark" : "light";
      document.documentElement.setAttribute("data-theme", resolvedTheme);
    };
    mediaQuery.addEventListener("change", handler);
    return () => mediaQuery.removeEventListener("change", handler);
  }

  // Called from +page.svelte onMount — two-stage boot
  async function init(tauriConnected: boolean) {
    // Stage 1: restore from localStorage immediately
    const saved = localStorage.getItem(STORAGE_KEY) || "system";
    await setTheme(saved, false);

    // Stage 2: reconcile with Tauri DB value if available
    if (tauriConnected) {
      try {
        const dbTheme = await invoke<string>("get_theme");
        if (dbTheme && dbTheme !== saved) {
          await setTheme(dbTheme, false);
        }
      } catch (e) {
        console.warn("Could not load theme from Tauri database:", e);
      }
    }
  }

  return {
    get currentTheme() { return currentTheme; },
    get resolvedTheme() { return resolvedTheme; },
    setTheme,
    init,
    initSystemListener,
  };
}

export const theme = createThemeStore();
