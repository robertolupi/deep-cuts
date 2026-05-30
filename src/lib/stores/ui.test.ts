import { describe, it, expect, beforeEach, vi } from "vitest";
import { ui } from "$lib/stores/ui.svelte";

function resetUiStore() {
  ui.activeView = 'table';
  ui.mapFocusTrackId = null;
  vi.resetAllMocks();
}

describe("UiStore — initial state", () => {
  beforeEach(resetUiStore);

  it("starts on the table view", () => {
    expect(ui.activeView).toBe('table');
  });

  it("starts with no map focus track", () => {
    expect(ui.mapFocusTrackId).toBeNull();
  });

  it("starts with empty toast messages", () => {
    expect(ui.errorMessage).toBe('');
    expect(ui.successMessage).toBe('');
  });
});

describe("UiStore — activeView", () => {
  beforeEach(resetUiStore);

  it("can be set to each valid view", () => {
    for (const view of ['table', 'map', 'analysis', 'settings'] as const) {
      ui.activeView = view;
      expect(ui.activeView).toBe(view);
    }
  });
});

describe("UiStore — focusMapTrack", () => {
  beforeEach(resetUiStore);

  it("sets mapFocusTrackId and switches to map view", () => {
    ui.focusMapTrack(42);
    expect(ui.mapFocusTrackId).toBe(42);
    expect(ui.activeView).toBe('map');
  });

  it("mapFocusTrackId can be cleared after use", () => {
    ui.focusMapTrack(7);
    ui.mapFocusTrackId = null;
    expect(ui.mapFocusTrackId).toBeNull();
  });
});

describe("UiStore — showToast", () => {
  beforeEach(() => {
    resetUiStore();
    vi.useFakeTimers();
  });

  it("sets errorMessage and clears successMessage on error", () => {
    ui.showToast("Something broke", "error");
    expect(ui.errorMessage).toBe("Something broke");
    expect(ui.successMessage).toBe('');
  });

  it("sets successMessage and clears errorMessage on success", () => {
    ui.showToast("All good", "success");
    expect(ui.successMessage).toBe("All good");
    expect(ui.errorMessage).toBe('');
  });

  it("clears messages after 4500ms", () => {
    ui.showToast("Temporary", "success");
    vi.advanceTimersByTime(4500);
    expect(ui.successMessage).toBe('');
    expect(ui.errorMessage).toBe('');
  });

  it("replaces a previous toast (debounces timeout)", () => {
    ui.showToast("First", "success");
    vi.advanceTimersByTime(2000);
    ui.showToast("Second", "error");

    expect(ui.errorMessage).toBe("Second");
    expect(ui.successMessage).toBe('');

    // First timer was cancelled — advancing another 2500ms should NOT clear
    vi.advanceTimersByTime(2500);
    expect(ui.errorMessage).toBe("Second");

    // Full 4500ms from second toast clears it
    vi.advanceTimersByTime(2000);
    expect(ui.errorMessage).toBe('');
  });
});
