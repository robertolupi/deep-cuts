<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { onDestroy, tick } from 'svelte';
  import { player } from '$lib/stores/player.svelte';

  type Message = { role: 'user' | 'assistant'; content: string };

  const track = $derived(player.selectedTrack);

  let messages    = $state<Message[]>([]);
  let streaming   = $state(false);
  let modelReady  = $state(false);
  let modelError  = $state('');
  let inputText   = $state('');
  let messagesEl  = $state<HTMLDivElement | null>(null);

  let unlistenToken: UnlistenFn | null = null;
  let currentTrackId: number | null = null;

  // When the track changes, reset conversation
  $effect(() => {
    const t = track;
    if (t?.id !== currentTrackId) {
      currentTrackId = t?.id ?? null;
      messages = [];
      modelReady = false;
      modelError = '';
      if (t) bootModel();
    }
  });

  async function bootModel() {
    modelReady = false;
    modelError = '';
    try {
      // ask_qwen will call ensure_llama_server_running internally; we just
      // issue a lightweight health check to reflect readiness in the UI.
      // For boot, we attempt a dummy ping via invoking ask_qwen with an empty
      // question won't work — instead we rely on the first real send to boot.
      // To show a spinner, we probe via a small ask on mount.
      modelReady = true; // optimistic — server boots on first send if not up
    } catch (e: any) {
      modelError = String(e);
    }
  }

  async function sendMessage() {
    if (!track || !inputText.trim() || streaming) return;

    const question = inputText.trim();
    inputText = '';

    messages = [...messages, { role: 'user', content: question }];
    messages = [...messages, { role: 'assistant', content: '' }];
    streaming = true;
    modelError = '';

    await tick();
    scrollToBottom();

    // Set up streaming token listener before invoking
    unlistenToken = await listen<string>('chat_token', (event) => {
      const last = messages.length - 1;
      if (last >= 0 && messages[last].role === 'assistant') {
        messages[last] = { ...messages[last], content: messages[last].content + event.payload };
        scrollToBottom();
      }
    });

    // Build history: all turns except the last assistant placeholder
    const history: [string, string][] = [];
    for (let i = 0; i + 1 < messages.length - 1; i += 2) {
      const u = messages[i];
      const a = messages[i + 1];
      if (u.role === 'user' && a.role === 'assistant') {
        history.push([u.content, a.content]);
      }
    }

    try {
      const response = await invoke<string>('ask_qwen', {
        trackId: track.id,
        question,
        windowStartSecs: null,
        windowDurationSecs: null,
        history,
      });

      // Replace placeholder with final response (streaming may have already filled it)
      const last = messages.length - 1;
      if (messages[last].role === 'assistant' && messages[last].content.length === 0) {
        messages[last] = { role: 'assistant', content: response };
      }
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

  onDestroy(() => {
    unlistenToken?.();
  });
</script>

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

    <!-- Messages -->
    <div class="messages" bind:this={messagesEl}>
      {#if messages.length === 0}
        <div class="conversation-hint">
          <p>Ask anything about this track — production, arrangement, mix, mood…</p>
          <div class="suggestions">
            <button class="suggestion" onclick={() => { inputText = "Why does the low end feel muddy?"; }}>Why does the low end feel muddy?</button>
            <button class="suggestion" onclick={() => { inputText = "What's the arrangement structure?"; }}>What's the arrangement structure?</button>
            <button class="suggestion" onclick={() => { inputText = "How does the mix compare to a typical club track?"; }}>How does the mix compare to a typical club track?</button>
          </div>
        </div>
      {/if}

      {#each messages as msg}
        <div class="message" class:user={msg.role === 'user'} class:assistant={msg.role === 'assistant'}>
          <span class="role-label">{msg.role === 'user' ? 'You' : 'Qwen'}</span>
          <div class="message-body">
            {#if msg.role === 'assistant' && streaming && msg === messages[messages.length - 1]}
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

    <!-- Input -->
    <div class="input-row">
      <textarea
        class="chat-input"
        placeholder="Ask something about this track…"
        bind:value={inputText}
        onkeydown={handleKeydown}
        disabled={streaming}
        rows="2"
      ></textarea>
      <button
        class="send-btn"
        onclick={sendMessage}
        disabled={streaming || !inputText.trim()}
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
