// Shared state for the dev inspector — lets Navbar open the drawer
// that is mounted in +layout.svelte.
// Only used in dev builds; tree-shaken by Vite in production.

class DevInspectorStore {
  open = $state(false);
  totalPending = $state(0);
}

export const devInspector = new DevInspectorStore();
