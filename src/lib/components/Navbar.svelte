<script lang="ts">
  import { theme } from "$lib/stores/theme.svelte";

  let {
    activeTab = $bindable(),
  }: {
    activeTab: string;
  } = $props();
</script>

<header class="navbar glass-panel">
  <div class="brand">
    <div class="logo-circle">
      <div class="logo-cut"></div>
    </div>
    <span class="brand-name shimmer-text">DEEP CUTS</span>
  </div>

  <!-- Navigation Tabs -->
  <nav class="nav-tabs">
    <button 
      class="nav-tab {activeTab === 'dashboard' ? 'active' : ''}" 
      onclick={() => activeTab = 'dashboard'}
    >
      Dashboard
    </button>
    <button
      class="nav-tab {activeTab === 'music-map' ? 'active' : ''}"
      onclick={() => activeTab = 'music-map'}
    >
      Music Map
    </button>
    <button
      class="nav-tab {activeTab === 'analysis' ? 'active' : ''}"
      onclick={() => activeTab = 'analysis'}
    >
      Analysis
    </button>
    <button
      class="nav-tab {activeTab === 'settings' ? 'active' : ''}"
      onclick={() => activeTab = 'settings'}
    >
      Settings
    </button>
  </nav>

  <!-- Theme Selection -->
  <div class="controls">
    <div class="theme-picker">
      <select
        class="theme-select"
        value={theme.currentTheme}
        onchange={(e) => theme.setTheme((e.target as HTMLSelectElement).value)}
        aria-label="Theme Selection"
      >
        <option value="system">🌓 System</option>
        <option value="dark">🌙 Dark Mode</option>
        <option value="light">☀️ Light Mode</option>
        <option value="accessible">👓 High Contrast</option>
      </select>
    </div>
  </div>
</header>
