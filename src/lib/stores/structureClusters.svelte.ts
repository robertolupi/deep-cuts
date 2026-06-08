import { invoke } from '$lib/ipc';

export interface StructureCluster {
  id: number;
  label: string;
  regex: string;
  track_count: number;
}

/**
 * @concept SAX
 * Manages symbolic audio structure clusters based on Symbolic Aggregate Approximation regex patterns.
 */
function createStructureClustersStore() {
  let clusters = $state<StructureCluster[]>([]);
  let loaded = $state(false);

  async function load() {
    if (loaded) return;
    try {
      clusters = await invoke('get_structure_clusters');
      loaded = true;
    } catch (e) {
      console.error('[structureClusters] failed to load:', e);
    }
  }

  const byId = $derived(
    Object.fromEntries(clusters.map(c => [c.id, c])) as Record<number, StructureCluster>
  );

  return {
    get clusters() { return clusters; },
    get byId() { return byId; },
    get loaded() { return loaded; },
    load,
  };
}

export const structureClusters = createStructureClustersStore();
