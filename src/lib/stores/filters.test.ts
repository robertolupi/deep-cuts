import { describe, it, expect, beforeEach } from "vitest";
import { filters } from "$lib/stores/filters.svelte";
import { library } from "$lib/stores/library.svelte";
import { MOCK_TRACKS, createTrack } from "../../test/fixtures";

function seedLibrary(tracks = MOCK_TRACKS) {
  library.tracks = [...tracks];
}

function resetFilters() {
  filters.searchQuery   = "";
  filters.genreFilter   = "";
  filters.minBpm        = 20;
  filters.maxBpm        = 250;
  filters.selectedKeys  = [];
  filters.selectedScale = "all";
  filters.musicOnly     = false;
  filters.vocalFilter   = "all";
  filters.clearSimilar();
}

describe("FiltersStore — initial state", () => {
  beforeEach(() => { resetFilters(); seedLibrary(); });

  it("starts with empty search and genre", () => {
    expect(filters.searchQuery).toBe("");
    expect(filters.genreFilter).toBe("");
  });

  it("starts with full BPM range", () => {
    expect(filters.minBpm).toBe(20);
    expect(filters.maxBpm).toBe(250);
  });

  it("starts with no keys and scale=all", () => {
    expect(filters.selectedKeys).toEqual([]);
    expect(filters.selectedScale).toBe("all");
  });

  it("filteredTracks returns all tracks when no filters are active", () => {
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });
});

describe("FiltersStore — genre filter", () => {
  beforeEach(() => { resetFilters(); seedLibrary(); });

  it("filters by metadata genre (case-insensitive, partial match)", () => {
    seedLibrary([
      createTrack({ id: 1, genre: "Electronic",  detected_genre: "electronic" }),
      createTrack({ id: 2, genre: "Ambient",     detected_genre: "ambient"    }),
      createTrack({ id: 3, genre: "Electronic",  detected_genre: "electronic" }),
      createTrack({ id: 4, genre: "Jazz",        detected_genre: "jazz"       }),
    ]);
    filters.genreFilter = "elect";
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1);
    expect(ids).toContain(3);
    expect(ids).not.toContain(2);
    expect(ids).not.toContain(4);
  });

  it("filters by detected_genre when metadata genre doesn't match", () => {
    seedLibrary([createTrack({ id: 10, genre: "Pop", detected_genre: "electronic" })]);
    filters.genreFilter = "electronic";
    expect(filters.filteredTracks).toHaveLength(1);
    expect(filters.filteredTracks[0].id).toBe(10);
  });

  it("returns empty list when no genre matches", () => {
    filters.genreFilter = "zzznomatch";
    expect(filters.filteredTracks).toHaveLength(0);
  });

  it("shows all tracks when genreFilter is whitespace only", () => {
    filters.genreFilter = "   ";
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });
});

describe("FiltersStore — key filter", () => {
  beforeEach(() => { resetFilters(); seedLibrary(); });

  it("shows all tracks when no keys selected and scale=all", () => {
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });

  it("filters to a single selected key note", () => {
    filters.selectedKeys = ["C"];
    const result = filters.filteredTracks;
    expect(result.every(t => t.key === "C")).toBe(true);
    expect(result).toHaveLength(1); // only id=1 has key C
  });

  it("ORs multiple selected keys", () => {
    filters.selectedKeys = ["C", "F"];
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1); // C major
    expect(ids).toContain(3); // F major
    expect(ids).not.toContain(2); // A minor
    expect(ids).not.toContain(4); // Bb minor
  });

  it("filters by scale=major only", () => {
    filters.selectedScale = "major";
    const result = filters.filteredTracks;
    expect(result.every(t => t.scale?.toLowerCase() === "major")).toBe(true);
  });

  it("filters by scale=minor only", () => {
    filters.selectedScale = "minor";
    const result = filters.filteredTracks;
    expect(result.every(t => t.scale?.toLowerCase() === "minor")).toBe(true);
  });

  it("combines key AND scale (e.g. A minor)", () => {
    filters.selectedKeys  = ["A"];
    filters.selectedScale = "minor";
    const result = filters.filteredTracks;
    expect(result.every(t => t.key === "A" && t.scale?.toLowerCase() === "minor")).toBe(true);
    expect(result).toHaveLength(1); // id=2 A minor
  });

  it("excludes tracks with null key when keys are selected", () => {
    filters.selectedKeys = ["C"];
    expect(filters.filteredTracks.map(t => t.id)).not.toContain(5); // null key
  });

  it("excludes tracks with null scale when scale filter is active", () => {
    filters.selectedScale = "major";
    expect(filters.filteredTracks.map(t => t.id)).not.toContain(5); // null scale
  });

  it("toggleKey adds a key", () => {
    filters.toggleKey("A");
    expect(filters.selectedKeys).toContain("A");
  });

  it("toggleKey removes a key that is already selected", () => {
    filters.selectedKeys = ["A", "C"];
    filters.toggleKey("A");
    expect(filters.selectedKeys).not.toContain("A");
    expect(filters.selectedKeys).toContain("C");
  });

  it("clearKeys resets to empty", () => {
    filters.selectedKeys = ["A", "C"];
    filters.clearKeys();
    expect(filters.selectedKeys).toEqual([]);
  });
});

describe("FiltersStore — BPM filter", () => {
  beforeEach(() => { resetFilters(); seedLibrary(); });

  it("shows all tracks when BPM range is at default", () => {
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });

  it("excludes tracks with null BPM when range is narrowed", () => {
    filters.minBpm = 60;
    filters.maxBpm = 200;
    expect(filters.filteredTracks.map(t => t.id)).not.toContain(5);
  });

  it("includes tracks whose BPM falls within the range", () => {
    filters.minBpm = 90;
    filters.maxBpm = 130;
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1);  // 128
    expect(ids).toContain(4);  // 95
    expect(ids).not.toContain(2); // 72
    expect(ids).not.toContain(3); // 145
  });

  it("handles minBpm === maxBpm (exact match)", () => {
    filters.minBpm = 128;
    filters.maxBpm = 128;
    const result = filters.filteredTracks;
    expect(result).toHaveLength(1);
    expect(result[0].bpm).toBe(128);
  });
});

describe("FiltersStore — search query filter", () => {
  beforeEach(() => { resetFilters(); seedLibrary(); });

  it("matches by title", () => {
    filters.searchQuery = "cyan";
    expect(filters.filteredTracks.map(t => t.id)).toContain(1);
  });

  it("matches by artist", () => {
    filters.searchQuery = "artist b";
    expect(filters.filteredTracks.map(t => t.id)).toContain(2);
  });

  it("matches by filename when title is absent", () => {
    seedLibrary([createTrack({ id: 20, title: null, filename: "my-rare-file.flac" })]);
    filters.searchQuery = "rare";
    expect(filters.filteredTracks).toHaveLength(1);
    expect(filters.filteredTracks[0].id).toBe(20);
  });

  it("is case-insensitive", () => {
    filters.searchQuery = "GLITCH";
    expect(filters.filteredTracks.map(t => t.id)).toContain(3);
  });

  it("returns empty when no track matches the query", () => {
    filters.searchQuery = "zzznomatch";
    expect(filters.filteredTracks).toHaveLength(0);
  });
});

describe("FiltersStore — AND logic across filters", () => {
  beforeEach(() => { resetFilters(); seedLibrary(); });

  it("applies genre AND BPM filters together", () => {
    filters.genreFilter = "electronic";
    filters.minBpm = 130;
    filters.maxBpm = 250;
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(3);     // Electronic 145 bpm
    expect(ids).not.toContain(1); // Electronic 128 bpm — below range
  });

  it("applies search AND key+scale filters together", () => {
    filters.searchQuery   = "artist a";
    filters.selectedKeys  = ["F"];
    filters.selectedScale = "major";
    // id=1 Artist A, C major — key excluded; id=3 Artist A, F major — passes
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(3);
    expect(ids).not.toContain(1);
  });
});

describe("FiltersStore — musicOnly filter", () => {
  beforeEach(() => { resetFilters(); });

  it("shows all tracks when musicOnly is false", () => {
    seedLibrary([
      createTrack({ id: 1, is_music: 1 }),
      createTrack({ id: 2, is_music: 0 }),
      createTrack({ id: 3, is_music: null }),
    ]);
    expect(filters.filteredTracks).toHaveLength(3);
  });

  it("shows only is_music=1 tracks when musicOnly is true", () => {
    seedLibrary([
      createTrack({ id: 1, is_music: 1 }),
      createTrack({ id: 2, is_music: 0 }),
      createTrack({ id: 3, is_music: null }),
    ]);
    filters.musicOnly = true;
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1);
    expect(ids).not.toContain(2);
    expect(ids).not.toContain(3);
  });

  it("excludes unanalyzed (null) tracks when musicOnly is true", () => {
    seedLibrary([createTrack({ id: 1, is_music: null })]);
    filters.musicOnly = true;
    expect(filters.filteredTracks).toHaveLength(0);
  });

  it("combines musicOnly with BPM filter", () => {
    seedLibrary([
      createTrack({ id: 1, is_music: 1,    bpm: 128 }),
      createTrack({ id: 2, is_music: 1,    bpm: 60  }),
      createTrack({ id: 3, is_music: null, bpm: 128 }),
    ]);
    filters.musicOnly = true;
    filters.minBpm    = 100;
    filters.maxBpm    = 250;
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1);
    expect(ids).not.toContain(2);
    expect(ids).not.toContain(3);
  });
});

describe("FiltersStore — vocalFilter", () => {
  beforeEach(() => { resetFilters(); });

  it("shows all tracks when vocalFilter is 'all'", () => {
    seedLibrary([
      createTrack({ id: 1, detected_vocal: "voice" }),
      createTrack({ id: 2, detected_vocal: "instrumental" }),
      createTrack({ id: 3, detected_vocal: null }),
    ]);
    expect(filters.filteredTracks).toHaveLength(3);
  });

  it("filters to voice tracks only", () => {
    seedLibrary([
      createTrack({ id: 1, detected_vocal: "voice" }),
      createTrack({ id: 2, detected_vocal: "instrumental" }),
      createTrack({ id: 3, detected_vocal: null }),
    ]);
    filters.vocalFilter = "voice";
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1);
    expect(ids).not.toContain(2);
    expect(ids).not.toContain(3);
  });

  it("filters to instrumental tracks only", () => {
    seedLibrary([
      createTrack({ id: 1, detected_vocal: "voice" }),
      createTrack({ id: 2, detected_vocal: "instrumental" }),
      createTrack({ id: 3, detected_vocal: null }),
    ]);
    filters.vocalFilter = "instrumental";
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(2);
    expect(ids).not.toContain(1);
    expect(ids).not.toContain(3);
  });

  it("excludes null detected_vocal when a specific filter is active", () => {
    seedLibrary([createTrack({ id: 1, detected_vocal: null })]);
    filters.vocalFilter = "voice";
    expect(filters.filteredTracks).toHaveLength(0);
  });

  it("combines vocalFilter with musicOnly", () => {
    seedLibrary([
      createTrack({ id: 1, is_music: 1,    detected_vocal: "voice" }),
      createTrack({ id: 2, is_music: 1,    detected_vocal: "instrumental" }),
      createTrack({ id: 3, is_music: null, detected_vocal: "voice" }),
    ]);
    filters.musicOnly   = true;
    filters.vocalFilter = "voice";
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1);
    expect(ids).not.toContain(2);
    expect(ids).not.toContain(3);
  });
});

describe("FiltersStore — similarTo filter", () => {
  beforeEach(() => { resetFilters(); });

  it("starts with no similarToTrack", () => {
    expect(filters.similarToTrack).toBeNull();
  });

  it("clearSimilar resets similarToTrack to null", () => {
    // Manually inject state as setSimilarTo is async (requires IPC)
    filters.clearSimilar();
    expect(filters.similarToTrack).toBeNull();
  });

  it("shows all tracks when no similar filter is active", () => {
    seedLibrary(MOCK_TRACKS);
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });

  it("isSimilarLoading starts false", () => {
    expect(filters.isSimilarLoading).toBe(false);
  });
});
