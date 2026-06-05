<script lang="ts" generics="T">
  import { onMount } from 'svelte';

  let {
    value = $bindable(""),
    options = [],
    placeholder = "Search...",
    onselect,
    onkeydown,
    onfocus,
    itemSnippet,
    buttonSnippet,
    borderless = false
  }: {
    value: string;
    options: T[];
    placeholder?: string;
    onselect: (option: T) => void;
    onkeydown?: (e: KeyboardEvent) => void;
    onfocus?: () => void;
    itemSnippet?: import('svelte').Snippet<[T]>;
    buttonSnippet?: import('svelte').Snippet;
    borderless?: boolean;
  } = $props();

  let showDropdown = $state(false);
  let activeIndex = $state(-1);
  let container = $state<HTMLDivElement | null>(null);
  let dropdownEl = $state<HTMLDivElement | null>(null);

  // Keyboard navigation
  function handleKeyDown(e: KeyboardEvent) {
    if (showDropdown && options.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        activeIndex = (activeIndex + 1) % options.length;
        scrollActiveIntoView();
        return;
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        activeIndex = (activeIndex - 1 + options.length) % options.length;
        scrollActiveIntoView();
        return;
      } else if (e.key === "Enter") {
        if (activeIndex >= 0 && activeIndex < options.length) {
          e.preventDefault();
          selectOption(options[activeIndex]);
          return;
        }
      } else if (e.key === "Escape") {
        showDropdown = false;
        activeIndex = -1;
      }
    }

    if (onkeydown) {
      onkeydown(e);
    }
  }

  function scrollActiveIntoView() {
    if (!dropdownEl) return;
    const items = dropdownEl.querySelectorAll('.autocomplete-item');
    const activeItem = items[activeIndex] as HTMLElement;
    if (activeItem && typeof activeItem.scrollIntoView === 'function') {
      activeItem.scrollIntoView({ block: 'nearest' });
    }
  }

  function selectOption(option: T) {
    onselect(option);
    showDropdown = false;
    activeIndex = -1;
  }

  function handleWindowClick(e: MouseEvent) {
    if (container && !container.contains(e.target as Node)) {
      showDropdown = false;
    }
  }
</script>

<svelte:window onclick={handleWindowClick} />

<div class="autocomplete-container" bind:this={container} onkeydown={handleKeyDown} style="position: relative; width: 100%; display: flex; gap: 4px;">
  <input
    type="text"
    {placeholder}
    bind:value
    onfocus={() => { showDropdown = true; if (onfocus) onfocus(); }}
    autocomplete="off"
    autocorrect="off"
    autocapitalize="none"
    spellcheck="false"
    style="flex-grow: 1; background: {borderless ? 'transparent' : 'var(--sg-surface-container)'}; border: {borderless ? 'none' : '1px solid var(--sg-surface-high)'}; border-radius: 4px; padding: {borderless ? '0' : '4px 8px'}; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: var(--sg-on-surface, #e3e1e9); outline: none;"
  />
  {#if buttonSnippet}
    {@render buttonSnippet()}
  {/if}

  {#if showDropdown && options.length > 0}
    <div 
      bind:this={dropdownEl} 
      class="autocomplete-dropdown" 
      style="position: absolute; bottom: 100%; left: 0; right: 0; background: var(--sg-surface-slate); border: 1px solid var(--sg-surface-highest); border-radius: 4px; max-height: 120px; overflow-y: auto; z-index: 150; margin-bottom: 4px; box-shadow: 0 -4px 12px var(--sg-surface-dim);"
    >
      {#each options as option, i}
        <button
          type="button"
          class="autocomplete-item"
          style="width: 100%; text-align: left; background: {i === activeIndex ? 'var(--sg-surface-high)' : 'none'}; border: none; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: {i === activeIndex ? 'var(--sg-primary, #00f0ff)' : 'var(--sg-on-surface, #e3e1e9)'}; padding: 6px 8px; cursor: pointer; display: block;"
          onclick={() => selectOption(option)}
          onmouseenter={() => activeIndex = i}
        >
          {#if itemSnippet}
            {@render itemSnippet(option)}
          {:else}
            {String(option)}
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
