<script lang="ts">
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { onDestroy, tick } from 'svelte';
  import WaveSurfer from 'wavesurfer.js';
  import RegionsPlugin from 'wavesurfer.js/dist/plugins/regions.esm.js';
  import { player } from '$lib/stores/player.svelte';
  import { library } from '$lib/stores/library.svelte';
  import { theme } from '$lib/stores/theme.svelte';

  type Message = { role: 'user' | 'assistant'; content: string };
  interface ChatSession { id: number; track_id: number; title: string; window_start_secs: number | null; window_duration_secs: number | null; created_at: number; updated_at: number; }
  interface ChatSearchResult { session_id: number; track_id: number; track_title: string; session_title: string; excerpt: string; }

  const MAX_REGION_SECS = 240; // 4 minutes — matches backend cap

  const track = $derived(player.selectedTrack);
  const analysisRunning = $derived(library.analysisRunning);

  let messages    = $state<Message[]>([]);
  let streaming   = $state(false);
  let modelReady  = $state(false);
  let modelError  = $state('');
  let inputText   = $state('');
  let messagesEl  = $state<HTMLDivElement | null>(null);

  // Region selector state
  let regionStart = $state(0);
  let regionEnd   = $state(0);
  let waveformEl  = $state<HTMLDivElement | null>(null);

  // Session state
  let currentSession    = $state<ChatSession | null>(null);
  let trackSessions     = $state<ChatSession[]>([]);
  let sessionQuery      = $state('');
  let searchResults     = $state<ChatSearchResult[]>([]);
  let sessionDropdown   = $state(false);
  let sessionInputEl    = $state<HTMLInputElement | null>(null);

  let currentTrackId: number | null = null;
  let unlistenToken: UnlistenFn | null = null;

  // WaveSurfer instance for the region selector (not for playback)
  let chatWs: WaveSurfer | null = null;

  function destroyChatWs() {
    if (chatWs) {
      chatWs.destroy();
      chatWs = null;
    }
  }

  // Build the peaks-only WaveSurfer with a single draggable region
  async function mountRegionSelector(trackPath: string, duration: number) {
    destroyChatWs();
    await tick();
    if (!waveformEl) return;

    const peaks = player.exportPeaks();

    const regionsPlugin = RegionsPlugin.create();

    const wsOpts: ConstructorParameters<typeof WaveSurfer>[0] = {
      container:     waveformEl,
      waveColor:     theme.resolvedTheme === 'light' ? 'rgba(28, 25, 23, 0.35)' : 'rgba(255,255,255,0.12)',
      progressColor: 'transparent',
      cursorWidth:   0,
      barWidth:      2,
      barGap:        1.5,
      barRadius:     1,
      height:        48,
      normalize:     true,
      interact:      false,
      plugins:       [regionsPlugin],
    };

    if (peaks && duration > 0) {
      // Peaks-only mode — no audio decode, renders instantly
      wsOpts.peaks    = peaks;
      wsOpts.duration = duration;
    } else {
      // Fallback: load from file (will decode, but no playback)
      wsOpts.url = convertFileSrc(trackPath);
    }

    chatWs = WaveSurfer.create(wsOpts);

    const initRegion = (dur: number) => {
      const end = Math.min(dur, MAX_REGION_SECS);
      regionStart = 0;
      regionEnd   = end;

      const region = regionsPlugin.addRegion({
        start:     0,
        end,
        color:     theme.resolvedTheme === 'light' ? 'rgba(13, 115, 119, 0.12)' : 'rgba(0, 240, 255, 0.12)',
        drag:      true,
        resize:    true,
        maxLength: MAX_REGION_SECS,
      });

      region.on('update-end', () => {
        regionStart = region.start;
        regionEnd   = region.end;
      });
    };

    if (peaks && duration > 0) {
      // Peaks mode fires 'ready' synchronously after create in v7
      chatWs.on('ready', (dur) => initRegion(dur || duration));
      // Also try immediately in case ready already fired
      const d = chatWs.getDuration();
      if (d > 0) initRegion(d);
    } else {
      chatWs.on('ready', (dur) => initRegion(dur));
    }
  }

  // ── Session helpers ──────────────────────────────────────────────────────

  function sessionLabel(s: ChatSession | null): string {
    if (!s) return '';
    return s.title;
  }

  async function loadTrackSessions(trackId: number) {
    trackSessions = await invoke<ChatSession[]>('list_chat_sessions', { trackId });
  }

  async function openSession(session: ChatSession) {
    currentSession = session;
    sessionQuery = '';
    searchResults = [];
    sessionDropdown = false;
    const msgs = await invoke<{ role: string; content: string }[]>('get_chat_messages', { sessionId: session.id });
    messages = msgs.map(m => ({ role: m.role as 'user' | 'assistant', content: m.content }));
    // Restore region if stored on the session
    if (session.window_start_secs != null && session.window_duration_secs != null) {
      regionStart = session.window_start_secs;
      regionEnd   = session.window_start_secs + session.window_duration_secs;
    }
    scrollToBottom();
  }

  async function deleteCurrentSession() {
    if (!currentSession || !track) return;
    await invoke('delete_chat_session', { sessionId: currentSession.id });
    await loadTrackSessions(track.id);
    if (trackSessions.length > 0) openSession(trackSessions[0]);
    else startNewSession();
  }

  function startNewSession() {
    currentSession = null;
    messages = [];
    sessionQuery = '';
    searchResults = [];
    sessionDropdown = false;
  }

  async function onSessionQueryInput() {
    const q = sessionQuery.trim();
    if (!q) {
      searchResults = [];
      return;
    }
    try {
      searchResults = await invoke<ChatSearchResult[]>('search_chats', { query: q + '*' });
    } catch { searchResults = []; }
  }

  async function onSearchResultClick(result: ChatSearchResult) {
    if (result.track_id !== track?.id) {
      const t = library.tracks.find(t => t.id === result.track_id);
      if (t) player.selectedTrack = t;
    }
    const sessions = await invoke<ChatSession[]>('list_chat_sessions', { trackId: result.track_id });
    await loadTrackSessions(result.track_id);
    const session = sessions.find(s => s.id === result.session_id);
    if (session) openSession(session);
  }

  // When the track changes, reset conversation and rebuild region selector
  $effect(() => {
    const t = track;
    if (t?.id !== currentTrackId) {
      currentTrackId = t?.id ?? null;
      messages = [];
      currentSession = null;
      sessionQuery = '';
      searchResults = [];
      modelReady = false;
      modelError = '';
      if (t) {
        bootModel();
        loadTrackSessions(t.id).then(() => {
          if (trackSessions.length > 0) openSession(trackSessions[0]);
        });
      } else {
        destroyChatWs();
        trackSessions = [];
      }
    }
  });

  // Re-build region selector when track or theme changes
  $effect(() => {
    const t = track;
    const _resolvedTheme = theme.resolvedTheme;
    if (t) {
      mountRegionSelector(t.path, player.duration);
    }
  });

  async function bootModel() {
    modelReady = false;
    modelError = '';
    try {
      modelReady = true; // optimistic — server boots on first send if not up
    } catch (e: any) {
      modelError = String(e);
    }
  }

  async function sendMessage() {
    if (!track || !inputText.trim() || streaming) return;

    const question = inputText.trim();
    inputText = '';

    // Lazily create a session on the first message
    if (!currentSession) {
      const windowDur = regionEnd - regionStart;
      currentSession = await invoke<ChatSession>('create_chat_session', {
        trackId: track.id,
        windowStartSecs:    regionStart,
        windowDurationSecs: windowDur > 0 ? windowDur : null,
      });
      trackSessions = [currentSession, ...trackSessions];
    }
    const sessionId = currentSession.id;

    messages = [...messages, { role: 'user', content: question }];
    messages = [...messages, { role: 'assistant', content: '' }];
    streaming = true;
    modelError = '';

    await tick();
    scrollToBottom();

    // Listen for streaming tokens before invoking so we don't miss early ones
    unlistenToken = await listen<string>('chat_token', (event) => {
      const last = messages.length - 1;
      if (last >= 0 && messages[last].role === 'assistant') {
        messages[last] = { ...messages[last], content: messages[last].content + event.payload };
        scrollToBottom();
      }
    });

    // Build history: all turns except the last (pending) assistant placeholder
    const history: [string, string][] = [];
    for (let i = 0; i + 1 < messages.length - 1; i += 2) {
      const u = messages[i];
      const a = messages[i + 1];
      if (u.role === 'user' && a.role === 'assistant') {
        history.push([u.content, a.content]);
      }
    }

    const windowDuration = regionEnd - regionStart;

    try {
      const response = await invoke<string>('ask_qwen', {
        trackId: track.id,
        question,
        windowStartSecs:    regionStart,
        windowDurationSecs: windowDuration > 0 ? windowDuration : null,
        history,
      });

      // Streaming may have already filled the bubble; only overwrite if empty
      const last = messages.length - 1;
      if (messages[last].content.length === 0) {
        messages[last] = { role: 'assistant', content: response };
      }

      // Persist both turns and refresh session list (auto-title may have updated)
      await invoke('save_chat_message', { sessionId, role: 'user',      content: question });
      await invoke('save_chat_message', { sessionId, role: 'assistant', content: messages[last].content });
      await loadTrackSessions(track.id);
      currentSession = trackSessions.find(s => s.id === sessionId) ?? currentSession;
    } catch (e: any) {
      const last = messages.length - 1;
      messages[last] = { role: 'assistant', content: `Error: ${e}` };
      modelError = String(e);
    } finally {
      streaming = false;
      unlistenToken?.();
      unlistenToken = null;
      scrollToBottom();
    }
  }

  onDestroy(() => {
    unlistenToken?.();
    destroyChatWs();
  });

  function scrollToBottom() {
    if (messagesEl) {
      messagesEl.scrollTop = messagesEl.scrollHeight;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  function formatSecs(s: number): string {
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60);
    return `${m}:${sec.toString().padStart(2, '0')}`;
  }
</script>

<svelte:window onclick={() => { sessionDropdown = false; }} />

<div class="chat-panel">
  {#if !track}
    <div class="empty-state">
      <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="empty-icon">
        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
      </svg>
      <p class="empty-title">No track selected</p>
      <p class="empty-sub">Select a track from the library to start a conversation about it.</p>
    </div>
  {:else}
    <!-- Track header -->
    <div class="track-header">
      <div class="track-info">
        <span class="track-name">{track.title || track.filename}</span>
        {#if track.artist}<span class="track-artist">{track.artist}</span>{/if}
      </div>
      {#if !modelReady}
        <div class="model-status">
          <span class="spinner"></span>
          <span class="model-status-text">Loading model…</span>
        </div>
      {/if}
    </div>

    <!-- Session combobox -->
    <div class="session-bar">
      <div class="session-row">
      <div class="session-combobox" class:open={sessionDropdown}>
        <input
          bind:this={sessionInputEl}
          class="session-input"
          type="text"
          placeholder={currentSession ? sessionLabel(currentSession) : 'New Chat'}
          bind:value={sessionQuery}
          oninput={onSessionQueryInput}
          onfocus={() => { sessionDropdown = true; }}
          onclick={(e) => e.stopPropagation()}
        />
        {#if sessionDropdown}
          <div class="session-dropdown" onclick={(e) => e.stopPropagation()}>
            {#if sessionQuery.trim()}
              <!-- FTS search results -->
              {#if searchResults.length === 0}
                <div class="session-empty">No results</div>
              {:else}
                {#each searchResults as result}
                  <button class="session-result" onclick={() => onSearchResultClick(result)}>
                    <span class="session-result-track">{result.track_title}</span>
                    <span class="session-result-excerpt">{result.excerpt}</span>
                  </button>
                {/each}
              {/if}
            {:else}
              <!-- Session list for this track -->
              <button class="session-item session-item-new" onclick={startNewSession} disabled={analysisRunning}>
                + New Chat
              </button>
              {#if trackSessions.length > 0}
                <div class="session-sep"></div>
                {#each trackSessions as session}
                  <button
                    class="session-item"
                    class:session-item-active={currentSession?.id === session.id}
                    onclick={() => openSession(session)}
                  >{session.title}</button>
                {/each}
              {/if}
            {/if}
          </div>
        {/if}
      </div>
      {#if currentSession}
        <button class="session-delete-btn" onclick={deleteCurrentSession} title="Delete this chat">
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4h6v2"/>
          </svg>
        </button>
      {/if}
      </div>
    </div>

    <!-- Region selector -->
    <div class="region-selector">
      <div class="region-label-row">
        <span class="region-label">Audio context</span>
        <span class="region-range">{formatSecs(regionStart)} – {formatSecs(regionEnd)}</span>
      </div>
      <div class="waveform-wrap" bind:this={waveformEl}></div>
      <div class="region-hint">Drag region to select which part of the track to analyse (max 4 min)</div>
    </div>

    <!-- Messages -->
    <div class="messages" bind:this={messagesEl}>
      {#if messages.length === 0}
        <div class="conversation-hint">
          <p>Ask anything about this track — production, arrangement, mix, mood…</p>
          <div class="suggestions">
            <button class="suggestion" onclick={() => { inputText = "What language are the vocals in? If instrumental, say so."; }}>What language are the vocals in?</button>
            <button class="suggestion" onclick={() => { inputText = "What era or decade does this track sound like?"; }}>What era or decade does this sound like?</button>
            <button class="suggestion" onclick={() => { inputText = "What's the best listening context for this track — focus, workout, party, background, sleep?"; }}>Best listening context?</button>
            <button class="suggestion" onclick={() => { inputText = "What are the main themes or subjects of the lyrics?"; }}>What are the lyrical themes?</button>
            <button class="suggestion" onclick={() => { inputText = "Does this sound like a studio recording or a live performance?"; }}>Studio or live recording?</button>
            <button class="suggestion" onclick={() => { inputText = "What's the arrangement structure of this track?"; }}>What's the arrangement structure?</button>
            <button class="suggestion" onclick={() => { inputText = "Why does the low end feel muddy?"; }}>Why does the low end feel muddy?</button>
            <button class="suggestion" onclick={() => { inputText = "How does the mix compare to a typical club track?"; }}>How does the mix compare to a club track?</button>
            <button class="suggestion" onclick={() => { inputText = "Give me arrangement tips to improve this song."; }}>Give me arrangement tips to improve this.</button>
          </div>
        </div>
      {/if}

      {#each messages as msg, i}
        <div class="message" class:user={msg.role === 'user'} class:assistant={msg.role === 'assistant'}>
          <span class="role-label">{msg.role === 'user' ? 'You' : 'Qwen'}</span>
          <div class="message-body">
            {#if msg.role === 'assistant' && msg.content === '' && streaming && i === messages.length - 1}
              <span class="waiting">
                <span class="spinner waiting-spinner"></span>
                {messages.length === 2 ? 'Analysing audio…' : 'Thinking…'}
              </span>
            {:else if msg.role === 'assistant' && streaming && i === messages.length - 1}
              {msg.content}<span class="cursor">▌</span>
            {:else}
              {msg.content}
            {/if}
          </div>
        </div>
      {/each}
    </div>

    {#if modelError}
      <div class="error-banner">{modelError}</div>
    {/if}

    {#if analysisRunning}
      <div class="analysis-running-banner">Analysis pipeline is running — chat is disabled to prevent llama-server conflicts. Wait for analysis to finish.</div>
    {/if}

    <!-- Input -->
    <div class="input-row">
      <textarea
        class="chat-input"
        placeholder="Ask something about this track…"
        bind:value={inputText}
        onkeydown={handleKeydown}
        disabled={streaming || analysisRunning}
        rows="2"
      ></textarea>
      <button
        class="send-btn"
        onclick={sendMessage}
        disabled={streaming || !inputText.trim() || analysisRunning}
        aria-label="Send"
      >
        {#if streaming}
          <span class="spinner"></span>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="22" y1="2" x2="11" y2="13"/>
            <polygon points="22 2 15 22 11 13 2 9 22 2"/>
          </svg>
        {/if}
      </button>
    </div>
  {/if}
</div>

<style>
  .chat-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--sg-surface);
    overflow: hidden;
  }

  /* ── Empty state ── */
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 2rem;
    color: var(--sg-outline, #849495);
    text-align: center;
  }

  .empty-icon {
    opacity: 0.3;
    color: var(--sg-primary, #00f0ff);
  }

  .empty-title {
    font-family: "JetBrains Mono", monospace;
    font-size: 13px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
    margin: 0;
  }

  .empty-sub {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-outline, #849495);
    max-width: 320px;
    line-height: 1.6;
    margin: 0;
  }

  /* ── Track header ── */
  .track-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    border-bottom: 1px solid rgba(255,255,255,0.07);
    background: var(--sg-surface-slate, #161b22);
    gap: 1rem;
  }

  .track-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .track-name {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-artist {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .model-status {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .model-status-text {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
  }

  /* ── Region selector ── */
  .region-selector {
    flex-shrink: 0;
    padding: 8px 16px 6px;
    border-bottom: 1px solid rgba(255,255,255,0.07);
    background: var(--sg-surface-slate, #161b22);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .region-label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .region-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
  }

  .region-range {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
    letter-spacing: 0.04em;
  }

  .waveform-wrap {
    width: 100%;
    height: 48px;
    border-radius: 3px;
    overflow: hidden;
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.06);
  }

  /* Override wavesurfer region handle colours */
  :global(.waveform-wrap .wavesurfer-region) {
    border-left:  2px solid rgba(0, 240, 255, 0.7) !important;
    border-right: 2px solid rgba(0, 240, 255, 0.7) !important;
  }

  :global(html[data-theme="light"] .waveform-wrap .wavesurfer-region) {
    border-left:  2px solid rgba(13, 115, 119, 0.7) !important;
    border-right: 2px solid rgba(13, 115, 119, 0.7) !important;
  }

  .region-hint {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    opacity: 0.6;
  }

  /* ── Messages ── */
  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    scroll-behavior: smooth;
  }

  .conversation-hint {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 0;
    color: var(--sg-outline, #849495);
  }

  .conversation-hint p {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    margin: 0;
    line-height: 1.5;
  }

  .suggestions {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .suggestion {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    text-align: left;
    padding: 6px 10px;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    background: rgba(255,255,255,0.03);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .suggestion:hover {
    border-color: rgba(0,240,255,0.3);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.05);
  }

  .message {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-width: 100%;
  }

  .role-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .message.user .role-label {
    color: var(--sg-primary, #00f0ff);
  }

  .message.assistant .role-label {
    color: var(--sg-secondary, #fe00fe);
  }

  .message-body {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    line-height: 1.65;
    color: var(--sg-on-surface, #e3e1e9);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .message.user .message-body {
    background: rgba(0,240,255,0.06);
    border-left: 2px solid var(--sg-primary, #00f0ff);
    padding: 8px 10px;
    border-radius: 0 4px 4px 0;
  }

  .message.assistant .message-body {
    background: rgba(255,255,255,0.03);
    border-left: 2px solid var(--sg-secondary, #fe00fe);
    padding: 8px 10px;
    border-radius: 0 4px 4px 0;
  }

  /* ── Waiting / streaming indicators ── */
  .waiting {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--sg-outline, #849495);
    font-style: italic;
  }

  .waiting-spinner {
    width: 10px;
    height: 10px;
    flex-shrink: 0;
  }

  .cursor {
    animation: blink 1s step-end infinite;
    color: var(--sg-secondary, #fe00fe);
  }

  @keyframes blink {
    50% { opacity: 0; }
  }

  /* ── Error ── */
  .error-banner {
    flex-shrink: 0;
    margin: 0 16px 8px;
    padding: 8px 10px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: #ff6b6b;
    background: rgba(255, 80, 80, 0.08);
    border: 1px solid rgba(255, 80, 80, 0.25);
    border-radius: 4px;
    word-break: break-word;
  }

  .analysis-running-banner {
    flex-shrink: 0;
    margin: 0 16px 8px;
    padding: 8px 10px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: #f0a030;
    background: rgba(240, 160, 48, 0.08);
    border: 1px solid rgba(240, 160, 48, 0.25);
    border-radius: 4px;
    word-break: break-word;
  }

  /* ── Input ── */
  .input-row {
    flex-shrink: 0;
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 10px 16px;
    border-top: 1px solid rgba(255,255,255,0.07);
    background: var(--sg-surface-slate, #161b22);
  }

  .chat-input {
    flex: 1;
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 4px;
    color: var(--sg-on-surface, #e3e1e9);
    padding: 8px 10px;
    resize: none;
    outline: none;
    line-height: 1.5;
    transition: border-color 0.15s;
  }

  .chat-input:focus {
    border-color: rgba(0,240,255,0.35);
  }

  .chat-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .send-btn {
    flex-shrink: 0;
    width: 34px;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0,240,255,0.1);
    border: 1px solid rgba(0,240,255,0.3);
    border-radius: 4px;
    color: var(--sg-primary, #00f0ff);
    cursor: pointer;
    transition: all 0.12s;
  }

  .send-btn:hover:not(:disabled) {
    background: rgba(0,240,255,0.18);
    border-color: var(--sg-primary, #00f0ff);
  }

  .send-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ── Session combobox ── */
  .session-bar {
    flex-shrink: 0;
    padding: 6px 16px;
    border-bottom: 1px solid rgba(255,255,255,0.07);
    background: var(--sg-surface-slate, #161b22);
  }

  .session-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .session-combobox {
    position: relative;
    flex: 1;
  }

  .session-delete-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: none;
    border: 1px solid transparent;
    border-radius: 4px;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .session-delete-btn:hover {
    border-color: rgba(255,80,80,0.4);
    color: #ff6060;
    background: rgba(255,60,60,0.07);
  }

  .session-input {
    width: 100%;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    background: rgba(255,255,255,0.03);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 4px;
    color: var(--sg-on-surface, #e3e1e9);
    padding: 5px 10px;
    outline: none;
    box-sizing: border-box;
    transition: border-color 0.15s;
  }

  .session-input:focus {
    border-color: rgba(0,240,255,0.3);
  }

  .session-input::placeholder {
    color: var(--sg-outline, #849495);
    font-style: italic;
  }

  .session-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    overflow: hidden;
    z-index: 200;
    max-height: 280px;
    overflow-y: auto;
  }

  .session-empty {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    padding: 10px 12px;
  }

  .session-item {
    display: block;
    width: 100%;
    text-align: left;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 7px 12px;
    background: none;
    border: none;
    color: var(--sg-on-surface, #e3e1e9);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background 0.1s;
  }

  .session-item:hover { background: rgba(255,255,255,0.05); }

  .session-item-new {
    color: var(--sg-primary, #00f0ff);
    font-weight: 700;
  }

  .session-item-active {
    background: rgba(0,240,255,0.06);
    color: var(--sg-primary, #00f0ff);
  }

  .session-sep {
    height: 1px;
    background: rgba(255,255,255,0.07);
    margin: 2px 0;
  }

  .session-result {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    padding: 7px 12px;
    background: none;
    border: none;
    cursor: pointer;
    transition: background 0.1s;
  }

  .session-result:hover { background: rgba(255,255,255,0.05); }

  .session-result-track {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .session-result-excerpt {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Spinner ── */
  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(0,240,255,0.2);
    border-top-color: var(--sg-primary, #00f0ff);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
