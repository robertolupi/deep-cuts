import { describe, expect, it } from "vitest";
import { resolveTrackColor, type MappedTrackPoint } from "./mapMath";

function makeTrack(overrides: Partial<MappedTrackPoint> = {}): MappedTrackPoint {
  return {
    id: 1,
    x: 50,
    y: 50,
    watched_directory_id: 1,
    title: "Test Track",
    filename: "test.mp3",
    artist: null,
    genre: null,
    bpm: null,
    key: null,
    scale: null,
    ...overrides,
  };
}

const themeColors = {
  bpmCool: "#0000ff",
  bpmHot: "#ff0000",
  dotBorder: "#ffffff",
  dotBorderWidth: 1,
  canvasBg: "#000000",
};

const genreColors: Record<string, string> = {
  Electronic: "#00ffff",
  Jazz: "#ff9900",
  Unknown: "#888888",
  Other: "#cccccc",
};

// ── mood coloring ─────────────────────────────────────────────────────────────

describe("resolveTrackColor — mood", () => {
  it("returns the color of the dominant mood", () => {
    const track = makeTrack({ mood_happy: 0.9, mood_sad: 0.1 });
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#ffeb3b");
  });

  it("picks aggressive over relaxed when aggressive is higher", () => {
    const track = makeTrack({ mood_aggressive: 0.8, mood_relaxed: 0.3 });
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#ff1744");
  });

  it("returns grey when all mood values are zero", () => {
    const track = makeTrack({
      mood_happy: 0, mood_sad: 0, mood_aggressive: 0,
      mood_relaxed: 0, mood_party: 0, mood_acoustic: 0, mood_electronic: 0,
    });
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#aaaaaa");
  });

  it("returns grey when all mood fields are null/undefined", () => {
    const track = makeTrack();
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#aaaaaa");
  });

  it("treats values at the 1e-5 threshold boundary as zero (grey)", () => {
    const track = makeTrack({ mood_party: 1e-5 });
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#aaaaaa");
  });

  it("returns party color when party is the sole non-zero mood", () => {
    const track = makeTrack({ mood_party: 0.7 });
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#d500f9");
  });

  it("returns electronic color correctly", () => {
    const track = makeTrack({ mood_electronic: 0.95 });
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#00e5ff");
  });

  it("returns acoustic color correctly", () => {
    const track = makeTrack({ mood_acoustic: 0.6, mood_happy: 0.1 });
    expect(resolveTrackColor(track, "mood", genreColors, themeColors)).toBe("#ff9100");
  });
});

// ── camelot coloring ──────────────────────────────────────────────────────────

describe("resolveTrackColor — camelot", () => {
  it("returns the correct color for A minor (8A)", () => {
    const track = makeTrack({ key: "A", scale: "minor" });
    expect(resolveTrackColor(track, "camelot", genreColors, themeColors)).toBe("#FF1744");
  });

  it("returns the correct color for C major (8B)", () => {
    const track = makeTrack({ key: "C", scale: "major" });
    expect(resolveTrackColor(track, "camelot", genreColors, themeColors)).toBe("#FFE082");
  });

  it("returns grey for an unknown key", () => {
    const track = makeTrack({ key: "X", scale: "major" });
    expect(resolveTrackColor(track, "camelot", genreColors, themeColors)).toBe("#aaaaaa");
  });

  it("returns grey when key is null", () => {
    const track = makeTrack({ key: null, scale: "minor" });
    expect(resolveTrackColor(track, "camelot", genreColors, themeColors)).toBe("#aaaaaa");
  });
});

// ── genre coloring ────────────────────────────────────────────────────────────

describe("resolveTrackColor — genre", () => {
  it("matches a known genre by substring (case-insensitive)", () => {
    const track = makeTrack({ genre: "Electronic" });
    expect(resolveTrackColor(track, "genre", genreColors, themeColors)).toBe("#00ffff");
  });

  it("uses the primary segment before a separator character", () => {
    const track = makeTrack({ genre: "Electronic / Dance" });
    expect(resolveTrackColor(track, "genre", genreColors, themeColors)).toBe("#00ffff");
  });

  it("falls back to Unknown color for empty genre", () => {
    const track = makeTrack({ genre: "" });
    expect(resolveTrackColor(track, "genre", genreColors, themeColors)).toBe("#888888");
  });

  it("falls back to Other color for unrecognised genre", () => {
    const track = makeTrack({ genre: "Polka" });
    expect(resolveTrackColor(track, "genre", genreColors, themeColors)).toBe("#cccccc");
  });
});

// ── bpm coloring ──────────────────────────────────────────────────────────────

describe("resolveTrackColor — bpm", () => {
  it("uses bpmCool color for very low bpm", () => {
    // pct = clamp((60 - 70) / 110, 0, 1) = 0 → pure bpmCool
    // d3.interpolateRgb returns "rgb(...)" strings at the endpoints
    const track = makeTrack({ bpm: 60 });
    const result = resolveTrackColor(track, "bpm", genreColors, themeColors);
    expect(result).toBe("rgb(0, 0, 255)");
  });

  it("uses bpmHot color for very high bpm", () => {
    // pct = clamp((200 - 70) / 110, 0, 1) = 1 → pure bpmHot
    const track = makeTrack({ bpm: 200 });
    const result = resolveTrackColor(track, "bpm", genreColors, themeColors);
    expect(result).toBe("rgb(255, 0, 0)");
  });

  it("defaults to bpm 120 when bpm is null", () => {
    const trackNull = makeTrack({ bpm: null });
    const trackExplicit = makeTrack({ bpm: 120 });
    expect(
      resolveTrackColor(trackNull, "bpm", genreColors, themeColors)
    ).toBe(
      resolveTrackColor(trackExplicit, "bpm", genreColors, themeColors)
    );
  });
});
