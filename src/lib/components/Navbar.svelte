<script lang="ts">
  import { theme } from "$lib/stores/theme.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { library } from "$lib/stores/library.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { devInspector } from "$lib/stores/devInspector.svelte";

  const analysisActive = $derived(library.analysisRunning && !library.analysisPaused);

  // ── Dev context menu ─────────────────────────────────────────────────────────
  let devMenuOpen = $state(false);
  let devMenuX = $state(0);
  let devMenuY = $state(0);

  // Acoustid batch state
  let acoustidRunning = $state(false);
  let acoustidProgress = $state('');

  function openDevMenu(e: MouseEvent) {
    if (!import.meta.env.DEV) return;
    e.preventDefault();
    devMenuX = e.clientX;
    devMenuY = e.clientY;
    devMenuOpen = true;
  }

  function closeDevMenu() { devMenuOpen = false; }

  async function runAcoustidBatch() {
    closeDevMenu();
    if (acoustidRunning) return;
    acoustidRunning = true;
    acoustidProgress = 'starting…';

    const unlisten = await listen<[number, number]>('acoustid-batch-progress', (e) => {
      const [done, total] = e.payload;
      acoustidProgress = `${done} / ${total}`;
    });
    const unlistenDone = await listen<[number, number]>('acoustid-batch-done', (e) => {
      const [done, total] = e.payload;
      acoustidProgress = `done — ${done} / ${total} enriched`;
      acoustidRunning = false;
      unlisten();
      unlistenDone();
    });

    await invoke('enrich_all_pending_acoustid').catch((err: unknown) => {
      acoustidProgress = `error: ${err}`;
      acoustidRunning = false;
      unlisten();
      unlistenDone();
    });
  }

  const views: { id: typeof ui.activeView; label: string }[] = [
    { id: 'table',      label: 'Library'    },
    { id: 'map',        label: 'Map'        },
    { id: 'duplicates', label: 'Duplicates' },
    { id: 'analysis',   label: 'Analyze'    },
    { id: 'statistics', label: 'Statistics' },
    { id: 'chat',       label: 'Chat'       },
    { id: 'settings',   label: 'Settings'   },
  ];
</script>

<header class="navbar">
  <!-- Wordmark (right-click for dev menu in debug builds) -->
  <span
    class="brand"
    oncontextmenu={openDevMenu}
    role="button"
    tabindex="-1"
  >DEEP CUTS</span>

  {#if import.meta.env.DEV}
    {#await import('$lib/components/dev/DevHud.svelte') then { default: DevHud }}
      <DevHud
        totalPending={devInspector.totalPending}
        onOpen={() => devInspector.open = true}
      />
    {/await}
  {/if}

  {#if import.meta.env.DEV && acoustidProgress}
    <span class="dev-status">{acoustidProgress}</span>
  {/if}

  {#if import.meta.env.DEV && devMenuOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="dev-backdrop" onclick={closeDevMenu}></div>
    <menu class="dev-menu" style="left:{devMenuX}px; top:{devMenuY}px">
      <li class="dev-menu-header">⚙ Dev tools</li>
      <li>
        <button class="dev-menu-item" onclick={() => { closeDevMenu(); devInspector.open = true; }}>
          Open inspector
        </button>
      </li>
      <li>
        <button class="dev-menu-item" onclick={runAcoustidBatch} disabled={acoustidRunning}>
          {acoustidRunning ? `AcoustID batch… ${acoustidProgress}` : 'Enrich pending tracks (AcoustID)'}
        </button>
      </li>
    </menu>
  {/if}

  <!-- View toggles -->
  <nav class="view-toggle">
    {#each views as v}
      <button
        class="vt-btn"
        class:vt-active={ui.activeView === v.id}
        class:vt-analysis-active={v.id === 'analysis' && analysisActive}
        onclick={() => ui.activeView = v.id}
      >
        {#if v.id === 'table'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/>
            <line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>
          </svg>
        {:else if v.id === 'map'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="3"/>
            <line x1="12" y1="2" x2="12" y2="9"/><line x1="12" y1="15" x2="12" y2="22"/>
            <line x1="2" y1="12" x2="9" y2="12"/><line x1="15" y1="12" x2="22" y2="12"/>
          </svg>
        {:else if v.id === 'duplicates'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
        {:else if v.id === 'analysis'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
          </svg>
        {:else if v.id === 'statistics'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/>
          </svg>
        {:else if v.id === 'chat'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3"/>
            <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14"/>
            <path d="M15.54 8.46a5 5 0 0 1 0 7.07M8.46 8.46a5 5 0 0 0 0 7.07"/>
          </svg>
        {/if}
        {v.label}
      </button>
    {/each}
  </nav>

  <!-- Theme picker -->
  <div class="theme-wrap">
    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="theme-icon">
      <circle cx="12" cy="12" r="5"/>
      <line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/>
      <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
      <line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/>
      <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
    </svg>
    <select
      class="theme-select"
      value={theme.currentTheme}
      onchange={(e) => theme.setTheme((e.target as HTMLSelectElement).value)}
      aria-label="Theme"
    >
      <option value="system">System</option>
      <option value="dark">Dark</option>
      <option value="light">Light</option>
      <option value="accessible">High Contrast</option>
    </select>
  </div>
</header>

<style>
  .navbar {
    flex-shrink: 0;
    height: 44px;
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0 1rem;
    background: var(--sg-surface-slate, #161b22);
    border-bottom: 1px solid rgba(255,255,255,0.07);
  }

  /* ── Wordmark ── */
  .brand {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    letter-spacing: 0.18em;
    color: var(--sg-primary, #00f0ff);
    flex-shrink: 0;
    text-shadow: 0 0 12px color-mix(in srgb, var(--sg-primary) 40%, transparent);
  }

  /* ── Dev status badge ── */
  .dev-status {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    color: var(--sg-warning, #f0a500);
    opacity: 0.8;
    flex-shrink: 0;
  }

  /* ── Dev context menu ── */
  .dev-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
  }

  .dev-menu {
    position: fixed;
    z-index: 1000;
    list-style: none;
    margin: 0;
    padding: 4px 0;
    min-width: 220px;
    background: var(--sg-surface-low);
    border: 1px solid color-mix(in srgb, var(--sg-primary) 25%, transparent);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.6); /* TODO: map to --sg-* token */
  }

  .dev-menu-header {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    color: var(--sg-primary, #00f0ff);
    padding: 6px 12px 4px;
    opacity: 0.7;
    pointer-events: none;
    border-bottom: 1px solid rgba(255,255,255,0.07);
    margin-bottom: 4px;
  }

  .dev-menu-item {
    display: block;
    width: 100%;
    text-align: left;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-on-surface, #e3e1e9);
    background: transparent;
    border: none;
    padding: 6px 14px;
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }

  .dev-menu-item:hover:not(:disabled) {
    background: rgba(0,240,255,0.08);
    color: var(--sg-primary, #00f0ff);
  }

  .dev-menu-item:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* ── View toggles ── */
  .view-toggle {
    display: flex;
    align-items: center;
    gap: 2px;
    margin: 0 auto;
    background: rgba(255,255,255,0.03);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 6px;
    padding: 3px;
  }

  .vt-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    padding: 5px 12px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
    white-space: nowrap;
  }

  .vt-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255,255,255,0.05);
  }

  .vt-active {
    background: rgba(0,240,255,0.1);
    color: var(--sg-primary, #00f0ff);
  }

  .vt-active:hover {
    background: rgba(0,240,255,0.14);
  }

  .vt-analysis-active {
    color: var(--sg-primary);
    text-shadow: 0 0 8px color-mix(in srgb, var(--sg-primary) 60%, transparent);
    animation: analysis-glow-pulse 2.4s ease-in-out infinite;
  }

  .vt-analysis-active svg {
    filter: drop-shadow(0 0 4px color-mix(in srgb, var(--sg-primary) 70%, transparent));
  }

  @keyframes analysis-glow-pulse {
    0%, 100% { text-shadow: 0 0 6px color-mix(in srgb, var(--sg-primary) 40%, transparent); }
    50%       { text-shadow: 0 0 14px color-mix(in srgb, var(--sg-primary) 85%, transparent); }
  }

  /* ── Theme picker ── */
  .theme-wrap {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
  }

  .theme-icon {
    color: var(--sg-outline, #849495);
    flex-shrink: 0;
  }

  .theme-select {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    background: rgba(255,255,255,0.03);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 4px;
    color: var(--sg-outline, #849495);
    padding: 3px 6px;
    outline: none;
    cursor: pointer;
    transition: border-color 0.15s, color 0.15s;
  }

  .theme-select:hover, .theme-select:focus {
    border-color: rgba(0,240,255,0.3);
    color: var(--sg-on-surface, #e3e1e9);
  }
</style>
