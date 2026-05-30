import { describe, it, expect, beforeEach } from "vitest";
import { filters } from "$lib/stores/filters.svelte";
import { library } from "$lib/stores/library.svelte";
import { MOCK_TRACKS, createTrack } from "../../test/fixtures";

// Seed library.tracks before each test so the derived filteredTracks has data
function seedLibrary(tracks = MOCK_TRACKS) {
  library.tracks = [...tracks];
}

describe("FiltersStore — initial state", () => {
  beforeEach(() => {
    filters.searchQuery = "";
    filters.genreFilter = "";
    filters.minBpm = 20;
    filters.maxBpm = 250;
    filters.selectedKey = "All";
    seedLibrary();
  });

  it("starts with empty search and genre", () => {
    expect(filters.searchQuery).toBe("");
    expect(filters.genreFilter).toBe("");
  });

  it("starts with full BPM range", () => {
    expect(filters.minBpm).toBe(20);
    expect(filters.maxBpm).toBe(250);
  });

  it("starts with All keys selected", () => {
    expect(filters.selectedKey).toBe("All");
  });

  it("filteredTracks returns all tracks when no filters are active", () => {
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });
});

describe("FiltersStore — genre filter", () => {
  beforeEach(() => {
    filters.searchQuery = "";
    filters.genreFilter = "";
    filters.minBpm = 20;
    filters.maxBpm = 250;
    filters.selectedKey = "All";
    seedLibrary();
  });

  it("filters by metadata genre (case-insensitive, partial match)", () => {
    // Override detected_genre so Ambient/Jazz tracks don't sneak through via detected_genre
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
    seedLibrary([
      createTrack({ id: 10, genre: "Pop", detected_genre: "electronic" }),
    ]);
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
  beforeEach(() => {
    filters.searchQuery = "";
    filters.genreFilter = "";
    filters.minBpm = 20;
    filters.maxBpm = 250;
    filters.selectedKey = "All";
    seedLibrary();
  });

  it("shows all tracks when key is All", () => {
    filters.selectedKey = "All";
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });

  it("filters to matching key+scale only", () => {
    filters.selectedKey = "C major";
    const result = filters.filteredTracks;
    expect(result.every(t => t.key === "C" && t.scale?.toLowerCase() === "major")).toBe(true);
    expect(result).toHaveLength(1);
  });

  it("excludes tracks with null key/scale", () => {
    filters.selectedKey = "C major";
    expect(filters.filteredTracks.map(t => t.id)).not.toContain(5); // No BPM Track has null key
  });
});

describe("FiltersStore — BPM filter", () => {
  beforeEach(() => {
    filters.searchQuery = "";
    filters.genreFilter = "";
    filters.minBpm = 20;
    filters.maxBpm = 250;
    filters.selectedKey = "All";
    seedLibrary();
  });

  it("shows all tracks when BPM range is at default", () => {
    expect(filters.filteredTracks).toHaveLength(MOCK_TRACKS.length);
  });

  it("excludes tracks with null BPM when range is narrowed", () => {
    filters.minBpm = 60;
    filters.maxBpm = 200;
    expect(filters.filteredTracks.map(t => t.id)).not.toContain(5); // null bpm
  });

  it("includes tracks whose BPM falls within the range", () => {
    filters.minBpm = 90;
    filters.maxBpm = 130;
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(1); // 128
    expect(ids).toContain(4); // 95
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
  beforeEach(() => {
    filters.searchQuery = "";
    filters.genreFilter = "";
    filters.minBpm = 20;
    filters.maxBpm = 250;
    filters.selectedKey = "All";
    seedLibrary();
  });

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
  beforeEach(() => {
    filters.searchQuery = "";
    filters.genreFilter = "";
    filters.minBpm = 20;
    filters.maxBpm = 250;
    filters.selectedKey = "All";
    seedLibrary();
  });

  it("applies genre AND BPM filters together", () => {
    filters.genreFilter = "electronic";
    filters.minBpm = 130;
    filters.maxBpm = 250;
    // id=1 Electronic 128bpm — below range; id=3 Electronic 145bpm — in range
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(3);
    expect(ids).not.toContain(1);
  });

  it("applies search AND key filters together", () => {
    filters.searchQuery = "artist a";
    filters.selectedKey = "F major";
    // id=1 Artist A, C major — key excluded; id=3 Artist A, F major — passes both
    const ids = filters.filteredTracks.map(t => t.id);
    expect(ids).toContain(3);
    expect(ids).not.toContain(1);
  });
});
