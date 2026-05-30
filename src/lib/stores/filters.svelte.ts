import type { Track } from "$lib/types";
import { library } from "$lib/stores/library.svelte";

function createFiltersStore() {
  let searchQuery = $state("");
  let genreFilter = $state("");
  let minBpm = $state(20);
  let maxBpm = $state(250);
  let selectedKey = $state("All");

  const filteredTracks = $derived.by(() => {
    return library.tracks.filter((t) => {
      if (genreFilter.trim()) {
        const q = genreFilter.trim().toLowerCase();
        const metaMatch = t.genre?.toLowerCase().includes(q) ?? false;
        const detectedMatch = t.detected_genre?.toLowerCase().includes(q) ?? false;
        if (!metaMatch && !detectedMatch) return false;
      }

      if (selectedKey !== "All") {
        if (!t.key || !t.scale) return false;
        const keyLabel = `${t.key} ${t.scale.toLowerCase()}`;
        if (keyLabel.toLowerCase() !== selectedKey.toLowerCase()) return false;
      }

      if (minBpm > 20 || maxBpm < 250) {
        if (t.bpm === null || t.bpm === undefined) return false;
        if (t.bpm < minBpm || t.bpm > maxBpm) return false;
      }

      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const matchesTitle = t.title?.toLowerCase().includes(query) ?? false;
        const matchesArtist = t.artist?.toLowerCase().includes(query) ?? false;
        const matchesAlbum = t.album?.toLowerCase().includes(query) ?? false;
        const matchesFilename = t.filename.toLowerCase().includes(query);
        return matchesTitle || matchesArtist || matchesAlbum || matchesFilename;
      }

      return true;
    });
  });

  return {
    get searchQuery() { return searchQuery; },
    set searchQuery(v: string) { searchQuery = v; },
    get genreFilter() { return genreFilter; },
    set genreFilter(v: string) { genreFilter = v; },
    get minBpm() { return minBpm; },
    set minBpm(v: number) { minBpm = v; },
    get maxBpm() { return maxBpm; },
    set maxBpm(v: number) { maxBpm = v; },
    get selectedKey() { return selectedKey; },
    set selectedKey(v: string) { selectedKey = v; },
    get filteredTracks() { return filteredTracks; },
  };
}

export const filters = createFiltersStore();
