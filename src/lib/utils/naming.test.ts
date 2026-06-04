import { describe, it, expect } from "vitest";
import { generateSmartName, type FilterState } from "./naming";
import { createDir } from "../../test/fixtures";

const DEFAULT_STATE: FilterState = {
  searchQuery: "",
  semanticQuery: "",
  clapQuery: "",
  genreFilter: "",
  minBpm: 20,
  maxBpm: 250,
  selectedKeys: [],
  selectedScale: "all",
  vocalFilter: "all",
  musicOnly: false,
  similarToTrack: null,
  selectedDirectoryIds: [],
};

const MOCK_DIRS = [
  createDir({ id: 1, name: "Studio Library" }),
  createDir({ id: 2, name: "Crate Digs" }),
];

describe("generateSmartName utility", () => {
  it("returns 'All Tracks' when no filters are active", () => {
    const name = generateSmartName(DEFAULT_STATE, MOCK_DIRS);
    expect(name).toBe("All Tracks");
  });

  it("prioritizes AI Vibe search and prepends ✨ emoji", () => {
    const state = { ...DEFAULT_STATE, semanticQuery: "chill lofi beats" };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("✨ chill lofi beats");
  });

  it("prioritizes AI Sound search and prepends 🎵 emoji", () => {
    const state = { ...DEFAULT_STATE, clapQuery: "fat analog synth bass" };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("🎵 fat analog synth bass");
  });

  it("uses similar seed track title and prepends ≈ prefix", () => {
    const state = { ...DEFAULT_STATE, similarToTrack: { title: "Stairway to Heaven" } };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("≈ Stairway to Heaven");
  });

  it("returns keyword search exactly without quotes", () => {
    const state = { ...DEFAULT_STATE, searchQuery: "remix" };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("remix");
  });

  it("displays folder names when selected", () => {
    const state = { ...DEFAULT_STATE, selectedDirectoryIds: [1, 2] };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("Studio Library, Crate Digs");
  });

  it("appends BPM range if customized", () => {
    const state = { ...DEFAULT_STATE, genreFilter: "Techno", minBpm: 124, maxBpm: 130 };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("Techno (124–130 BPM)");
  });

  it("appends keys and scales properly", () => {
    const state = {
      ...DEFAULT_STATE,
      genreFilter: "House",
      selectedKeys: ["C#", "F#"],
      selectedScale: "minor" as const,
    };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("House [C#,F# Min]");
  });

  it("appends scale-only indicators", () => {
    const state = { ...DEFAULT_STATE, selectedScale: "major" as const };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("[Maj Keys]");
  });

  it("appends vocal details when filtering for voice or instrumental", () => {
    const vocalState = { ...DEFAULT_STATE, genreFilter: "Pop", vocalFilter: "voice" as const };
    expect(generateSmartName(vocalState, MOCK_DIRS)).toBe("Pop (Vocals)");

    const instrumentalState = { ...DEFAULT_STATE, genreFilter: "Ambient", vocalFilter: "instrumental" as const };
    expect(generateSmartName(instrumentalState, MOCK_DIRS)).toBe("Ambient (Instrumental)");
  });

  it("ignores musicOnly flag in generated name", () => {
    const state = { ...DEFAULT_STATE, genreFilter: "Jazz", musicOnly: true };
    const name = generateSmartName(state, MOCK_DIRS);
    expect(name).toBe("Jazz");
  });
});
