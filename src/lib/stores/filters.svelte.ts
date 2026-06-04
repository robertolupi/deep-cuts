import type { Track } from "$lib/types";
import { library } from "$lib/stores/library.svelte";
import { ui } from "$lib/stores/ui.svelte";
import { curation } from "$lib/stores/curation.svelte";
import { invoke } from "@tauri-apps/api/core";
import { generateSmartName } from "$lib/utils/naming";

export type ScaleFilter = "all" | "major" | "minor";

function createFiltersStore() {
  let searchQuery  = $state("");
  let semanticQuery = $state("");
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
  // 0.0 = pure feels (description/semantic), 1.0 = pure sounds (CLAP/acoustic), default 0.5
  let similarBlend = $state(0.5);

  let semanticTrackIds = $state<Set<number>>(new Set());
  let semanticTrackScores = $state<Map<number, number>>(new Map());
  let isSemanticLoading = $state(false);

  let moodHappyMin      = $state(0); let moodHappyMax      = $state(1);
  let moodSadMin        = $state(0); let moodSadMax        = $state(1);
  let moodAggressiveMin = $state(0); let moodAggressiveMax = $state(1);
  let moodRelaxedMin    = $state(0); let moodRelaxedMax    = $state(1);
  let moodPartyMin      = $state(0); let moodPartyMax      = $state(1);
  let moodAcousticMin   = $state(0); let moodAcousticMax   = $state(1);
  let moodElectronicMin = $state(0); let moodElectronicMax = $state(1);

  let selectedTags = $state<string[]>([]);

  let clapQuery = $state("");
  let clapTrackIds = $state<Set<number>>(new Set());
  let clapTrackScores = $state<Map<number, number>>(new Map());
  let isClapLoading = $state(false);

  const filteredTracks = $derived.by(() => {
    const results = library.tracks.filter((t) => {
      // Playlist filter
      if (curation.activePlaylist) {
        const playlistTrackIds = new Set(
          curation.activePlaylistTracks
            .map((pt) => pt.track_id)
            .filter((id) => id !== null) as number[]
        );
        if (!playlistTrackIds.has(t.id)) return false;
      }

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

      // Mood
      const moodChecks: [number | null | undefined, number, number][] = [
        [t.mood_happy,      moodHappyMin,      moodHappyMax],
        [t.mood_sad,        moodSadMin,        moodSadMax],
        [t.mood_aggressive, moodAggressiveMin, moodAggressiveMax],
        [t.mood_relaxed,    moodRelaxedMin,    moodRelaxedMax],
        [t.mood_party,      moodPartyMin,      moodPartyMax],
        [t.mood_acoustic,   moodAcousticMin,   moodAcousticMax],
        [t.mood_electronic, moodElectronicMin, moodElectronicMax],
      ];
      for (const [val, lo, hi] of moodChecks) {
        if (lo > 0 || hi < 1) {
          if (val == null) return false;
          if (val < lo || val > hi) return false;
        }
      }

      // Full-text search (Keyword)
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const matchesTitle    = t.title?.toLowerCase().includes(query) ?? false;
        const matchesArtist   = t.artist?.toLowerCase().includes(query) ?? false;
        const matchesAlbum    = t.album?.toLowerCase().includes(query) ?? false;
        const matchesFilename = t.filename.toLowerCase().includes(query);
        const matchesComposer = t.composer?.toLowerCase().includes(query) ?? false;
        if (!matchesTitle && !matchesArtist && !matchesAlbum && !matchesFilename && !matchesComposer) return false;
      }

      // Semantic AI search
      if (semanticQuery.trim()) {
        if (!semanticTrackIds.has(t.id)) return false;
      }

      // CLAP Sonic AI search
      if (clapQuery.trim()) {
        if (!clapTrackIds.has(t.id)) return false;
      }

      // Tag filter (AND — track must have every selected tag)
      if (selectedTags.length > 0) {
        const trackTags = library.trackTagMap.get(t.id) ?? [];
        if (!selectedTags.every(tag => trackTags.includes(tag))) return false;
      }

      return true;
    });

    const hasSemanticScores = semanticQuery.trim() && semanticTrackScores.size > 0;
    const hasClapScores     = clapQuery.trim()     && clapTrackScores.size > 0;
    if (hasSemanticScores || hasClapScores) {
      results.sort((a, b) => {
        const avgScore = (id: number) => {
          const scores: number[] = [];
          if (hasSemanticScores && semanticTrackScores.has(id)) scores.push(semanticTrackScores.get(id)!);
          if (hasClapScores     && clapTrackScores.has(id))     scores.push(clapTrackScores.get(id)!);
          return scores.length ? scores.reduce((s, v) => s + v, 0) / scores.length : 0;
        };
        return avgScore(b.id) - avgScore(a.id);
      });
    } else if (curation.activePlaylist) {
      // Sort by manual playlist position
      const posMap = new Map(curation.activePlaylistTracks.map(pt => [pt.track_id, pt.position]));
      results.sort((a, b) => {
        const posA = posMap.get(a.id) ?? 999999;
        const posB = posMap.get(b.id) ?? 999999;
        return posA - posB;
      });
    }

    return results;
  });

  const autoName = $derived.by(() => {
    return generateSmartName({
      searchQuery,
      semanticQuery,
      clapQuery,
      genreFilter,
      minBpm,
      maxBpm,
      selectedKeys,
      selectedScale,
      vocalFilter,
      musicOnly,
      similarToTrack,
      selectedDirectoryIds
    }, library.directories);
  });

  async function runSemanticSearch(q: string) {
    const queryText = q.trim();
    if (!queryText) {
      semanticTrackIds = new Set();
      semanticTrackScores = new Map();
      return;
    }
    isSemanticLoading = true;
    try {
      const results = await invoke<{ id: number; score: number }[]>(
        "search_semantic_tracks", { query: queryText }
      );
      semanticTrackIds = new Set(results.map(r => r.id));
      semanticTrackScores = new Map(results.map(r => [r.id, r.score]));
    } catch (err: any) {
      ui.showToast(`Semantic search failed: ${err?.toString() ?? "unknown error"}`, "error");
      semanticTrackIds = new Set();
      semanticTrackScores = new Map();
    } finally {
      isSemanticLoading = false;
    }
  }

  async function runClapSearch(q: string) {
    const queryText = q.trim();
    if (!queryText) {
      clapTrackIds = new Set();
      clapTrackScores = new Map();
      return;
    }
    isClapLoading = true;
    try {
      const results = await invoke<{ id: number; score: number }[]>(
        "search_clap_tracks", { query: queryText }
      );
      clapTrackIds = new Set(results.map(r => r.id));
      clapTrackScores = new Map(results.map(r => [r.id, r.score]));
    } catch (err: any) {
      ui.showToast(`Sonic similarity search failed: ${err?.toString() ?? "unknown error"}`, "error");
      clapTrackIds = new Set();
      clapTrackScores = new Map();
    } finally {
      isClapLoading = false;
    }
  }

  let semanticDebounceTimeout: any;
  function debouncedSemanticSearch(v: string) {
    clearTimeout(semanticDebounceTimeout);
    semanticDebounceTimeout = setTimeout(() => {
      runSemanticSearch(v);
    }, 350);
  }

  let clapDebounceTimeout: any;
  function debouncedClapSearch(v: string) {
    clearTimeout(clapDebounceTimeout);
    clapDebounceTimeout = setTimeout(() => {
      runClapSearch(v);
    }, 350);
  }

  return {
    get searchQuery()   { return searchQuery; },
    set searchQuery(v)  { searchQuery = v; },
    get semanticQuery()   { return semanticQuery; },
    set semanticQuery(v)  {
      semanticQuery = v;
      debouncedSemanticSearch(v);
    },
    get clapQuery()   { return clapQuery; },
    set clapQuery(v)  {
      clapQuery = v;
      debouncedClapSearch(v);
    },
    get genreFilter()   { return genreFilter; },
    set genreFilter(v)  { genreFilter = v; },
    get minBpm()        { return minBpm; },
    set minBpm(v)       { minBpm = v; },
    get maxBpm()        { return maxBpm; },
    set maxBpm(v)       { maxBpm = v; },
    get moodHappyMin()       { return moodHappyMin; },      set moodHappyMin(v)      { moodHappyMin = v; },
    get moodHappyMax()       { return moodHappyMax; },      set moodHappyMax(v)      { moodHappyMax = v; },
    get moodSadMin()         { return moodSadMin; },        set moodSadMin(v)        { moodSadMin = v; },
    get moodSadMax()         { return moodSadMax; },        set moodSadMax(v)        { moodSadMax = v; },
    get moodAggressiveMin()  { return moodAggressiveMin; }, set moodAggressiveMin(v) { moodAggressiveMin = v; },
    get moodAggressiveMax()  { return moodAggressiveMax; }, set moodAggressiveMax(v) { moodAggressiveMax = v; },
    get moodRelaxedMin()     { return moodRelaxedMin; },    set moodRelaxedMin(v)    { moodRelaxedMin = v; },
    get moodRelaxedMax()     { return moodRelaxedMax; },    set moodRelaxedMax(v)    { moodRelaxedMax = v; },
    get moodPartyMin()       { return moodPartyMin; },      set moodPartyMin(v)      { moodPartyMin = v; },
    get moodPartyMax()       { return moodPartyMax; },      set moodPartyMax(v)      { moodPartyMax = v; },
    get moodAcousticMin()    { return moodAcousticMin; },   set moodAcousticMin(v)   { moodAcousticMin = v; },
    get moodAcousticMax()    { return moodAcousticMax; },   set moodAcousticMax(v)   { moodAcousticMax = v; },
    get moodElectronicMin()  { return moodElectronicMin; }, set moodElectronicMin(v) { moodElectronicMin = v; },
    get moodElectronicMax()  { return moodElectronicMax; }, set moodElectronicMax(v) { moodElectronicMax = v; },
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
    get similarBlend()     { return similarBlend; },
    set similarBlend(v: number) { similarBlend = v; },
    get isSimilarLoading() { return isSimilarLoading; },
    get filteredTracks()   { return filteredTracks; },
    get autoName()         { return autoName; },
    get isSemanticLoading() { return isSemanticLoading; },
    get semanticTrackScores() { return semanticTrackScores; },
    get isClapLoading() { return isClapLoading; },
    get clapTrackScores() { return clapTrackScores; },

    async setSimilarTo(track: { id: number; title: string; }) {
      isSimilarLoading = true;
      try {
        const results = await invoke<{ id: number; distance: number }[]>(
          'search_similar_tracks_audio', { trackId: track.id, clapWeight: similarBlend }
        );
        similarTrackIds = new Set(results.map(r => r.id));
        similarToTrack  = track;
      } catch (err: any) {
        ui.showToast(`Similarity search failed: ${err?.toString() ?? 'unknown error'}`, 'error');
      } finally {
        isSimilarLoading = false;
      }
    },

    async setSimilarBlend(blend: number) {
      similarBlend = blend;
      if (!similarToTrack) return;
      isSimilarLoading = true;
      try {
        const results = await invoke<{ id: number; distance: number }[]>(
          'search_similar_tracks_audio', { trackId: similarToTrack.id, clapWeight: blend }
        );
        similarTrackIds = new Set(results.map(r => r.id));
      } catch (err: any) {
        ui.showToast(`Similarity search failed: ${err?.toString() ?? 'unknown error'}`, 'error');
      } finally {
        isSimilarLoading = false;
      }
    },

    clearSimilar() {
      similarToTrack  = null;
      similarTrackIds = new Set();
      similarBlend    = 0.5;
    },

    toggleKey(key: string) {
      selectedKeys = selectedKeys.includes(key)
        ? selectedKeys.filter(k => k !== key)
        : [...selectedKeys, key];
    },
    clearKeys() { selectedKeys = []; },

    get selectedTags() { return selectedTags; },
    toggleTag(tag: string) {
      selectedTags = selectedTags.includes(tag)
        ? selectedTags.filter(t => t !== tag)
        : [...selectedTags, tag];
    },
    clearTags() { selectedTags = []; },

    clearAll() {
      searchQuery          = "";
      semanticQuery        = "";
      clapQuery            = "";
      genreFilter          = "";
      minBpm               = 20;
      maxBpm               = 250;
      moodHappyMin = 0;      moodHappyMax = 1;
      moodSadMin = 0;        moodSadMax = 1;
      moodAggressiveMin = 0; moodAggressiveMax = 1;
      moodRelaxedMin = 0;    moodRelaxedMax = 1;
      moodPartyMin = 0;      moodPartyMax = 1;
      moodAcousticMin = 0;   moodAcousticMax = 1;
      moodElectronicMin = 0; moodElectronicMax = 1;
      selectedKeys         = [];
      selectedScale        = "all";
      musicOnly            = false;
      vocalFilter          = "all";
      selectedDirectoryIds = [];
      similarToTrack       = null;
      similarTrackIds      = new Set();
      semanticTrackIds     = new Set();
      semanticTrackScores  = new Map();
      clapTrackIds         = new Set();
      clapTrackScores      = new Map();
      selectedTags         = [];
    },
  };
}

export const filters = createFiltersStore();
