import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import TrackList from "./TrackList.svelte";
import { filters } from "$lib/stores/filters.svelte";
import { library } from "$lib/stores/library.svelte";
import { createTrack } from "../../test/fixtures";

const TRACKS = [
  createTrack({ id: 1, title: "Alpha" }),
  createTrack({ id: 2, title: "Beta" }),
  createTrack({ id: 3, title: "Gamma" }),
];

function mountList(selectedTrack = null as (typeof TRACKS)[0] | null) {
  return render(TrackList, {
    props: {
      selectedTrack,
      isPlaying: false,
      onTrackSelect: () => {},
    },
  });
}

beforeEach(() => {
  library.tracks = [...TRACKS];
  filters.searchQuery  = "";
  filters.genreFilter  = "";
  filters.minBpm       = 20;
  filters.maxBpm       = 250;
  filters.musicOnly    = false;
  filters.vocalFilter  = "all";
  filters.selectedKeys = [];
  filters.clearSimilar();
});

// ── Outside-filter banner ─────────────────────────────────────────────────────

describe("TrackList — outside-filter banner", () => {
  it("does not show the banner when no track is selected", () => {
    mountList(null);
    expect(screen.queryByText(/hidden by active filters/)).toBeNull();
  });

  it("does not show the banner when selected track is visible in the list", () => {
    mountList(TRACKS[0]);
    expect(screen.queryByText(/hidden by active filters/)).toBeNull();
  });

  it("shows the banner when selected track is filtered out", () => {
    filters.searchQuery = "zzznomatch";
    mountList(TRACKS[0]);
    expect(screen.getByText(/hidden by active filters/)).toBeInTheDocument();
  });

  it("banner includes the track title", () => {
    filters.searchQuery = "zzznomatch";
    mountList(TRACKS[0]);
    expect(screen.getByText(/"Alpha" is hidden by active filters/)).toBeInTheDocument();
  });

  it("banner uses the filename when title is absent", () => {
    filters.searchQuery = "zzznomatch";
    const noTitle = createTrack({ id: 4, title: null as any, filename: "untitled.mp3" });
    library.tracks = [...TRACKS, noTitle];
    mountList(noTitle);
    expect(screen.getByText(/"untitled.mp3" is hidden by active filters/)).toBeInTheDocument();
  });

  it("Clear filters button calls filters.clearAll and hides the banner", async () => {
    filters.searchQuery = "zzznomatch";
    mountList(TRACKS[0]);

    const btn = screen.getByRole("button", { name: /clear filters/i });
    await fireEvent.click(btn);

    expect(filters.searchQuery).toBe("");
    expect(screen.queryByText(/hidden by active filters/)).toBeNull();
  });
});

// ── Track rows ────────────────────────────────────────────────────────────────

describe("TrackList — track rows", () => {
  it("renders a row for each filtered track", () => {
    mountList();
    expect(screen.getByText("Alpha")).toBeInTheDocument();
    expect(screen.getByText("Beta")).toBeInTheDocument();
    expect(screen.getByText("Gamma")).toBeInTheDocument();
  });

  it("shows empty state when no tracks match", () => {
    filters.searchQuery = "zzznomatch";
    mountList();
    expect(screen.getByText(/No Matching Tracks Found/)).toBeInTheDocument();
  });

  it("shows library empty state when library has no tracks", () => {
    library.tracks = [];
    mountList();
    expect(screen.getByText(/Your Music Library is Empty/)).toBeInTheDocument();
  });
});
