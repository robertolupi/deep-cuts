# F1b: Typed CommandMap in ipc.ts

Source: [codebase-improvements.md](codebase-improvements.md)

## Status

The `CommandMap` type skeleton and overloaded `invoke()` are already in `src/lib/ipc.ts` (commit from this campaign). The 86 command entries are stubs — most have `args: unknown` and `result: unknown`.

## Goal

Fill in precise `args` and `result` types for every command in `CommandMap`. Then update call sites to remove the `invoke<T>` generic where the type can now be inferred.

## Approach

1. For each command, cross-reference:
   - The Rust handler signature in `src-tauri/src/commands/*.rs`
   - The existing TypeScript types in `src/lib/types.ts`
2. Replace `unknown` stubs with real types.
3. Tighten call sites where the generic is now redundant.
4. Add/update frontend store tests that exercise typed `invoke()`.

## Files to touch

- `src/lib/ipc.ts` — `CommandMap` entries
- `src/lib/types.ts` — may need new or refined types
- `src/lib/stores/*.svelte.ts` — call sites to tighten
- `src/lib/components/*.svelte` — call sites to tighten

## Notes

Mostly mechanical but needs careful alignment between Rust and TypeScript types. Can be done in parallel domain-by-domain (library, analysis, playlists, download, settings). `*.test.ts` files are exempt from the lint_ipc_imports check but should still use the typed `invoke`.
