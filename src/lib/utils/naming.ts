import type { WatchedDirectory } from "$lib/types";

export interface FilterState {
  searchQuery: string;
  semanticQuery: string;
  clapQuery: string;
  genreFilter: string;
  minBpm: number;
  maxBpm: number;
  selectedKeys: string[];
  selectedScale: "all" | "major" | "minor";
  vocalFilter: "all" | "voice" | "instrumental";
  musicOnly: boolean;
  similarToTrack: { title: string } | null;
  selectedDirectoryIds: number[];
}

export function generateSmartName(state: FilterState, directories: WatchedDirectory[]): string {
  const parts: string[] = [];

  // 1. Core Focus (Prioritized)
  if (state.semanticQuery.trim()) {
    parts.push(`✨ ${state.semanticQuery.trim()}`);
  } else if (state.clapQuery.trim()) {
    parts.push(`🎵 ${state.clapQuery.trim()}`);
  } else if (state.similarToTrack) {
    parts.push(`≈ ${state.similarToTrack.title}`);
  } else if (state.searchQuery.trim()) {
    parts.push(state.searchQuery.trim());
  } else if (state.genreFilter.trim()) {
    parts.push(state.genreFilter.trim());
  } else if (state.selectedDirectoryIds.length > 0) {
    const names = state.selectedDirectoryIds
      .map(id => directories.find(d => d.id === id)?.name)
      .filter(Boolean);
    if (names.length > 0) {
      parts.push(names.join(", "));
    }
  }

  // 2. BPM Range
  if (state.minBpm > 20 || state.maxBpm < 250) {
    parts.push(`(${Math.round(state.minBpm)}–${Math.round(state.maxBpm)} BPM)`);
  }

  // 3. Key & Scale
  if (state.selectedKeys.length > 0 || state.selectedScale !== "all") {
    const keysStr = state.selectedKeys.length > 0 ? state.selectedKeys.join(",") : "";
    const scaleStr = state.selectedScale !== "all" ? (state.selectedScale === "major" ? "Maj" : "Min") : "";
    if (keysStr && scaleStr) {
      parts.push(`[${keysStr} ${scaleStr}]`);
    } else if (keysStr) {
      parts.push(`[${keysStr}]`);
    } else if (scaleStr) {
      parts.push(`[${scaleStr} Keys]`);
    }
  }

  // 4. Vocals
  if (state.vocalFilter === "voice") {
    parts.push("(Vocals)");
  } else if (state.vocalFilter === "instrumental") {
    parts.push("(Instrumental)");
  }

  return parts.join(" ") || "All Tracks";
}
