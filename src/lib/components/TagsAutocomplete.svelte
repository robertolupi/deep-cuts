<script lang="ts">
  import { invoke } from "$lib/ipc";
  import { library } from "$lib/stores/library.svelte";
  import Autocomplete from "./Autocomplete.svelte";

  let {
    value = $bindable(""),
    placeholder = "Add tag...",
    excludeTags = [],
    onselect,
    onkeydown,
    buttonSnippet,
    borderless = false
  }: {
    value: string;
    placeholder?: string;
    excludeTags?: string[];
    onselect: (tag: string) => void;
    onkeydown?: (e: KeyboardEvent) => void;
    buttonSnippet?: import('svelte').Snippet;
    borderless?: boolean;
  } = $props();

  let allTagsLive = $state<string[]>(library.allTags);

  async function handleFocus() {
    try {
      allTagsLive = await invoke("get_all_tags");
    } catch (e) {
      // Fallback to library store if offline
    }
  }

  const suggestions = $derived.by(() => {
    const q = value.trim().toLowerCase();
    if (!q) return [];
    return allTagsLive
      .filter(t => t.toLowerCase().includes(q) && !excludeTags.includes(t))
      .slice(0, 12);
  });
</script>

{#snippet tagItem(suggestion: string)}
  <span style="display: flex; justify-content: space-between; align-items: center; width: 100%;">
    <span>{suggestion.split(':').slice(1).join(':')}</span>
    <span style="font-size: var(--sg-text-3xs); opacity: 0.5; margin-left: 4px; font-family: sans-serif;">{suggestion.split(':')[0]}</span>
  </span>
{/snippet}

<Autocomplete
  bind:value
  options={suggestions}
  {placeholder}
  onfocus={handleFocus}
  onselect={onselect}
  {onkeydown}
  itemSnippet={tagItem}
  {buttonSnippet}
  {borderless}
/>
