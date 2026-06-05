<script lang="ts">
  import { library } from "$lib/stores/library.svelte";
  import Autocomplete from "./Autocomplete.svelte";

  let {
    value = $bindable(""),
    placeholder = "Filter by genre...",
    onselect,
    onkeydown
  }: {
    value: string;
    placeholder?: string;
    onselect?: (genre: string) => void;
    onkeydown?: (e: KeyboardEvent) => void;
  } = $props();

  // All distinct genres in library
  const allGenres = $derived.by(() => {
    const set = new Set<string>();
    for (const t of library.tracks) {
      if (t.genre) {
        for (const g of t.genre.split(/[,;]/)) {
          const s = g.trim();
          if (s) set.add(s);
        }
      }
      if (t.detected_genre) set.add(t.detected_genre);
      if (t.ai_genre) set.add(t.ai_genre);
    }
    return Array.from(set).sort();
  });

  const suggestions = $derived(
    value.trim().length > 0
      ? allGenres.filter(g => g.toLowerCase().includes(value.trim().toLowerCase()))
      : []
  );
</script>

<Autocomplete
  bind:value
  options={suggestions}
  {placeholder}
  onselect={(val) => {
    value = val;
    if (onselect) onselect(val);
  }}
  {onkeydown}
/>
