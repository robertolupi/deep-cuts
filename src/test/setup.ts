/**
 * Vitest global setup — runs before every test file.
 * - Extends expect with @testing-library/jest-dom matchers
 * - Mocks Tauri APIs so stores can be imported in a plain jsdom context
 * - Mocks WaveSurfer.js (no audio/canvas in jsdom)
 */
import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// ── Tauri API mocks ────────────────────────────────────────────────────────────
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ── WaveSurfer mock ────────────────────────────────────────────────────────────
vi.mock("wavesurfer.js", () => ({
  default: {
    create: vi.fn(() => ({
      load: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      playPause: vi.fn(),
      destroy: vi.fn(),
      getDuration: vi.fn(() => 180),
      setOptions: vi.fn(),
      on: vi.fn(),
    })),
  },
}));

vi.mock("wavesurfer.js/dist/plugins/spectrogram.esm.js", () => ({
  default: {
    create: vi.fn(() => ({})),
  },
}));

// ── Canvas mock ────────────────────────────────────────────────────────────────
// jsdom does not implement canvas; mock getContext so player gradient code is silent
Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
  value: () => ({
    createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
  }),
  writable: true,
});
