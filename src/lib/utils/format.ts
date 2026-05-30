/**
 * Format utilities — shared across player store and components.
 * Extracted from +page.svelte as part of Phase 1.1 store refactor.
 */

/** Format seconds as m:ss */
export function formatDuration(sec: number): string {
  const mins = Math.floor(sec / 60);
  const secs = Math.floor(sec % 60);
  return `${mins}:${secs < 10 ? "0" : ""}${secs}`;
}

/** Format bytes as "x.x MB" */
export function formatSize(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
