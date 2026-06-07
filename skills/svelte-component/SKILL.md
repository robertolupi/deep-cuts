---
name: svelte-component
description: Conventions for writing Svelte 5 components in deep-cuts — runes, stores, props, event listeners, and file layout
---

# Svelte 5 Component Conventions

Deep Cuts uses **Svelte 5 with runes** throughout. Do not use the legacy `writable`/`readable` store API or `$:` reactive statements — use runes everywhere.

---

## File layout

| What | Where |
|------|-------|
| Components | `src/lib/components/ComponentName.svelte` |
| Global stores | `src/lib/stores/*.svelte.ts` (class-based, exported as singletons) |
| Types | `src/lib/types.ts` |
| Utility functions | `src/lib/utils/*.ts` |
| Routes/pages | `src/routes/` |

Store files use the `.svelte.ts` extension so runes compile correctly outside `.svelte` files.

---

## Runes cheatsheet

```svelte
<script lang="ts">
  // Props — declare with $props()
  let { track, onSelect }: { track: Track; onSelect: (t: Track) => void } = $props();

  // Local reactive state
  let isOpen = $state(false);

  // Derived (computed) values
  let label = $derived(track.title ?? track.filename);

  // Side-effects that re-run when dependencies change
  $effect(() => {
    console.log('track changed:', track.id);
  });

  // {@const} must be an immediate child of a block tag, not a DOM element
</script>
```

**`$effect` replaces `$:` and `onMount`.** Use it for any reactive side-effect. Use `untrack()` to read a value without subscribing to it:

```svelte
<script lang="ts">
  import { untrack } from 'svelte';

  $effect(() => {
    const id = track.id;                     // subscribes
    const current = untrack(() => isOpen);   // does NOT subscribe
  });
</script>
```

---

## Accessing global stores

All stores are pre-instantiated singletons. Import them directly — do not `new` them in components:

```svelte
<script lang="ts">
  import { library } from '$lib/stores/library.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { filters } from '$lib/stores/filters.svelte';
  import { player } from '$lib/stores/player.svelte';
</script>

<!-- Use store properties directly — they are already reactive $state fields -->
<p>{library.trackCount} tracks</p>
<button onclick={() => ui.setActiveView('map')}>Map</button>
```

Store classes use `$state` fields and plain methods. There is no `.subscribe()` or `$store` sigil.

---

## Store pattern (for new stores)

Use a **class with `$state` fields**, exported as a singleton:

```typescript
// src/lib/stores/my-store.svelte.ts
class MyStore {
  value = $state(0);
  doubled = $derived(this.value * 2);

  increment() {
    this.value++;
  }
}

export const myStore = new MyStore();
```

For stores that need more encapsulation, use a function factory (see `ui.svelte.ts` for the pattern):

```typescript
function createUiStore() {
  let activeView = $state<ActiveView>('table');
  // ... methods ...
  return { get activeView() { return activeView; }, setActiveView };
}
export const ui = createUiStore();
```

---

## Tauri event listeners in components

Use `$effect` with an async unlisten pattern. The listener is automatically cleaned up when the effect re-runs or the component is destroyed:

```svelte
<script lang="ts">
  import { listen } from '$lib/ipc';

  $effect(() => {
    let unlisten: (() => void) | undefined;
    listen<{ percent: number }>('my-event', (e) => {
      progress = e.payload.percent;
    }).then(fn => { unlisten = fn; });

    return () => unlisten?.();  // cleanup
  });
</script>
```

For persistent app-wide listeners (scan progress, analysis events), prefer wiring them into the appropriate store's `init()` method instead of individual components.

Stores that register Tauri listeners must be idempotent. Keep an `initialized` flag, retain every unlisten function, and expose a `dispose()` method for tests and hot-reload cleanup. Avoid hidden cross-store writes; route shared updates through explicit store methods.

## IPC access

App components and stores should import `invoke` and `listen` from `$lib/ipc`. Do not import directly from `@tauri-apps/api/core` or `@tauri-apps/api/event` unless you are adding a low-level wrapper in `src/lib/ipc.ts`. This keeps local-debug mode, browser-only UI debugging, and tests consistent.

---

## Common mistakes

| Mistake | Fix |
|---------|-----|
| `{@const}` inside a DOM element | Move it to an immediate child of `{#each}` / `{#if}` |
| `$:` reactive statement | Replace with `$derived` (value) or `$effect` (side-effect) |
| `writable()`/ `readable()` | Use a class with `$state` fields |
| `import { get } from 'svelte/store'` | Not needed — access store properties directly |
| Direct Tauri `invoke` / `listen` imports in app code | Import from `$lib/ipc` so mocks and typed wrappers stay centralized |
| Store `init()` adds listeners every call | Make `init()` idempotent and keep unlisten functions for `dispose()` |
| Forgetting `lang="ts"` on `<script>` | Add it — the project is fully TypeScript |
| New IPC command without a `CommandMap` entry | Add the command to `CommandMap` in `src/lib/ipc.ts` |
