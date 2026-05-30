import { describe, it, expect, vi, beforeEach } from "vitest";
import { theme } from "$lib/stores/theme.svelte";

// Tauri invoke is mocked globally in src/test/setup.ts

// jsdom does not implement matchMedia — provide a default stub
function stubMatchMedia(prefersDark: boolean) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn((query: string) => ({
      matches: prefersDark && query.includes("dark"),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  });
}

function resetThemeStore() {
  document.documentElement.removeAttribute("data-theme");
  localStorage.removeItem("deep-cuts-theme");
  stubMatchMedia(true); // default: system = dark
  vi.resetAllMocks();
}

describe("ThemeStore — setTheme", () => {
  beforeEach(resetThemeStore);

  it("sets currentTheme and persists to localStorage", async () => {
    await theme.setTheme("dark", false);
    expect(theme.currentTheme).toBe("dark");
    expect(localStorage.getItem("deep-cuts-theme")).toBe("dark");
  });

  it("sets resolvedTheme to the explicit value for non-system themes", async () => {
    await theme.setTheme("light", false);
    expect(theme.resolvedTheme).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });

  it("sets resolvedTheme to dark for system when prefers-color-scheme is dark", async () => {
    stubMatchMedia(true);
    await theme.setTheme("system", false);
    expect(theme.currentTheme).toBe("system");
    expect(theme.resolvedTheme).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("sets resolvedTheme to light for system when prefers-color-scheme is light", async () => {
    stubMatchMedia(false);
    await theme.setTheme("system", false);
    expect(theme.resolvedTheme).toBe("light");
  });

  it("calls invoke save_theme when saveToDb=true and tauriConnected=true", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await theme.setTheme("dark", true, true);

    expect(invoke).toHaveBeenCalledWith("save_theme", { theme: "dark" });
  });

  it("does not call invoke when saveToDb=false", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();

    await theme.setTheme("dark", false, true);

    expect(invoke).not.toHaveBeenCalledWith("save_theme", expect.anything());
  });

  it("does not call invoke when tauriConnected=false", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();

    await theme.setTheme("dark", true, false);

    expect(invoke).not.toHaveBeenCalledWith("save_theme", expect.anything());
  });

  it("does not throw if invoke rejects during save", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockRejectedValueOnce(new Error("db error"));

    await expect(theme.setTheme("accessible", true, true)).resolves.toBeUndefined();
  });
});

describe("ThemeStore — init", () => {
  beforeEach(resetThemeStore);

  it("loads theme from localStorage on first stage", async () => {
    localStorage.setItem("deep-cuts-theme", "light");
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce("light");

    await theme.init(false);

    expect(theme.currentTheme).toBe("light");
  });

  it("reconciles with DB theme if different from localStorage", async () => {
    localStorage.setItem("deep-cuts-theme", "dark");
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce("accessible");

    await theme.init(true);

    expect(theme.currentTheme).toBe("accessible");
  });

  it("keeps localStorage theme when DB returns the same value", async () => {
    localStorage.setItem("deep-cuts-theme", "dark");
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce("dark");

    await theme.init(true);

    expect(theme.currentTheme).toBe("dark");
  });

  it("skips DB query when tauriConnected=false", async () => {
    localStorage.setItem("deep-cuts-theme", "dark");
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();

    await theme.init(false);

    expect(invoke).not.toHaveBeenCalledWith("get_theme");
  });

  it("falls back to system theme when localStorage is empty", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValueOnce("system");

    await theme.init(false);

    expect(theme.currentTheme).toBe("system");
  });
});

describe("ThemeStore — initSystemListener", () => {
  beforeEach(resetThemeStore);

  it("returns a cleanup function that removes the event listener", () => {
    const removeEventListener = vi.fn();
    window.matchMedia = vi.fn(() => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener,
    })) as any;

    const cleanup = theme.initSystemListener();
    cleanup();

    expect(removeEventListener).toHaveBeenCalled();
  });
});
