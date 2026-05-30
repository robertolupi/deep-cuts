import type { Track } from "$lib/types";
import { library } from "$lib/stores/library.svelte";
import { ui } from "$lib/stores/ui.svelte";
import { invoke } from "@tauri-apps/api/core";

export type ScaleFilter = "all" | "major" | "minor";

function createFiltersStore() {
  let searchQuery  = $state("");
  let genreFilter  = $state("");
  let minBpm       = $state(20);
  let maxBpm       = $state(250);
  let selectedKeys = $state<string[]>([]);   // note names e.g. ["A", "C#"]
  let selectedScale = $state<ScaleFilter>("all");
  let musicOnly = $state(false);
  let vocalFilter = $state<"all" | "voice" | "instrumental">("all");
  let selectedDirectoryIds = $state<number[]>([]);
  let similarToTrack = $state<{ id: number; title: string } | null>(null);
  let similarTrackIds = $state<Set<number>>(new Set());
  let isSimilarLoading = $state(false);

  const filteredTracks = $derived.by(() => {
    return library.tracks.filter((t) => {
      // Sounds similar filter
      if (similarTrackIds.size > 0 && !similarTrackIds.has(t.id)) return false;

      // Music only: exclude tracks Essentia classified as Non-Music
      if (musicOnly && t.detected_genre?.startsWith("Non-Music")) return false;

      // Vocal / instrumental
      if (vocalFilter !== "all") {
        if (t.detected_vocal !== vocalFilter) return false;
      }

      // Watched directory
      if (selectedDirectoryIds.length > 0) {
        if (!selectedDirectoryIds.includes(t.watched_directory_id)) return false;
      }

      // Genre
      if (genreFilter.trim()) {
        const q = genreFilter.trim().toLowerCase();
        const metaMatch     = t.genre?.toLowerCase().includes(q) ?? false;
        const detectedMatch = t.detected_genre?.toLowerCase().includes(q) ?? false;
        if (!metaMatch && !detectedMatch) return false;
      }

      // Key note names (multi-select OR)
      if (selectedKeys.length > 0) {
        if (!t.key) return false;
        if (!selectedKeys.includes(t.key)) return false;
      }

      // Scale (major / minor)
      if (selectedScale !== "all") {
        if (!t.scale) return false;
        if (t.scale.toLowerCase() !== selectedScale) return false;
      }

      // BPM
      if (minBpm > 20 || maxBpm < 250) {
        if (t.bpm === null || t.bpm === undefined) return false;
        if (t.bpm < minBpm || t.bpm > maxBpm) return false;
      }

      // Full-text search
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const matchesTitle    = t.title?.toLowerCase().includes(query) ?? false;
        const matchesArtist   = t.artist?.toLowerCase().includes(query) ?? false;
        const matchesAlbum    = t.album?.toLowerCase().includes(query) ?? false;
        const matchesFilename = t.filename.toLowerCase().includes(query);
        return matchesTitle || matchesArtist || matchesAlbum || matchesFilename;
      }

      return true;
    });
  });

  return {
    get searchQuery()   { return searchQuery; },
    set searchQuery(v)  { searchQuery = v; },
    get genreFilter()   { return genreFilter; },
    set genreFilter(v)  { genreFilter = v; },
    get minBpm()        { return minBpm; },
    set minBpm(v)       { minBpm = v; },
    get maxBpm()        { return maxBpm; },
    set maxBpm(v)       { maxBpm = v; },
    get selectedKeys()  { return selectedKeys; },
    set selectedKeys(v) { selectedKeys = v; },
    get selectedScale() { return selectedScale; },
    set selectedScale(v: ScaleFilter) { selectedScale = v; },
    get musicOnly()        { return musicOnly; },
    set musicOnly(v)       { musicOnly = v; },
    get vocalFilter()          { return vocalFilter; },
    set vocalFilter(v: "all" | "voice" | "instrumental") { vocalFilter = v; },
    get selectedDirectoryIds() { return selectedDirectoryIds; },
    toggleDirectoryId(id: number) {
      selectedDirectoryIds = selectedDirectoryIds.includes(id)
        ? selectedDirectoryIds.filter(d => d !== id)
        : [...selectedDirectoryIds, id];
    },
    clearDirectories() { selectedDirectoryIds = []; },
    get similarToTrack()   { return similarToTrack; },
    get isSimilarLoading() { return isSimilarLoading; },
    get filteredTracks()   { return filteredTracks; },

    async setSimilarTo(track: { id: number; title: string; }) {
      isSimilarLoading = true;
      try {
        const results = await invoke<{ id: number; distance: number }[]>(
          'search_similar_tracks_audio', { trackId: track.id }
        );
        similarTrackIds = new Set(results.map(r => r.id));
        similarToTrack  = track;
      } catch (err: any) {
        ui.showToast(`Similarity search failed: ${err?.toString() ?? 'unknown error'}`, 'error');
      } finally {
        isSimilarLoading = false;
      }
    },

    clearSimilar() {
      similarToTrack  = null;
      similarTrackIds = new Set();
    },

    toggleKey(key: string) {
      selectedKeys = selectedKeys.includes(key)
        ? selectedKeys.filter(k => k !== key)
        : [...selectedKeys, key];
    },
    clearKeys() { selectedKeys = []; },

    clearAll() {
      searchQuery          = "";
      genreFilter          = "";
      minBpm               = 20;
      maxBpm               = 250;
      selectedKeys         = [];
      selectedScale        = "all";
      musicOnly            = false;
      vocalFilter          = "all";
      selectedDirectoryIds = [];
      similarToTrack       = null;
      similarTrackIds      = new Set();
    },
  };
}

export const filters = createFiltersStore();
