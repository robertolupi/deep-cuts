import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  MOCK_DIRECTORIES,
  MOCK_TRACKS,
  MOCK_TAGS,
  MOCK_ALL_TAGS,
  MOCK_PLAYLISTS,
  MOCK_SAVED_SEARCHES,
} from "$lib/mock-data";

export const LOCAL_DEBUG = typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).has("local_debug");

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const MOCK_RESPONSES: Record<string, (args?: any) => unknown> = {
  get_watched_directories: () => MOCK_DIRECTORIES,
  get_track_count: () => MOCK_TRACKS.length,
  get_tracks: () => MOCK_TRACKS,
  get_track: ({ trackId }: { trackId: number }) =>
    MOCK_TRACKS.find((t) => t.id === trackId) ?? null,
  get_all_track_tags: () => MOCK_TAGS,
  get_all_tags: () => MOCK_ALL_TAGS,
  get_playlists: () => MOCK_PLAYLISTS,
  get_saved_searches: () => MOCK_SAVED_SEARCHES,
  get_playlist_tracks: () => [],
  get_playlists_for_track: () => [],
  is_analysis_running: () => false,
  get_analysis_paused_status: () => ({ manually_paused: false, auto_paused: false }),
  get_theme: () => "system",
};

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (LOCAL_DEBUG) {
    const handler = MOCK_RESPONSES[cmd];
    if (handler) return Promise.resolve(handler(args) as T);
    // Unknown commands silently resolve to undefined in mock mode
    console.warn(`[local_debug] unhandled invoke: ${cmd}`);
    return Promise.resolve(undefined as T);
  }
  return tauriInvoke<T>(cmd, args);
}

export function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void
): Promise<UnlistenFn> {
  if (LOCAL_DEBUG) {
    // No real events in mock mode — return a no-op unlisten
    return Promise.resolve(() => {});
  }
  return tauriListen<T>(event, handler);
}
