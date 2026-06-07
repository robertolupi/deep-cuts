<script lang="ts">
  import SettingsCard from "./SettingsCard.svelte";

  interface Props {
    appVersion: string;
    onOpenLicense: () => void;
  }
  let { appVersion, onOpenLicense }: Props = $props();
</script>

<SettingsCard title="About Deep Cuts" subtitle={appVersion ? `v${appVersion}` : undefined} class="about-card">
  <div class="about-body">
    <img src="/deep_cuts_transparent.png" alt="Deep Cuts logo" class="about-logo" />
    <p class="about-desc">
      A local-first music intelligence desktop app that analyzes your audio library with machine
      learning — BPM, key, genre, mood, and semantic embeddings — so producers can filter, search,
      and discover reference tracks by sonic characteristics. Everything runs on your machine,
      with no cloud dependency.
    </p>
    <div class="about-meta">
    <span class="about-copyright">© 2025 <a class="about-link" href="https://www.rlupi.com" target="_blank" rel="noopener noreferrer">Roberto Lupi</a></span>
    <span class="about-sep">·</span>
    <span class="about-license-sep">·</span>
    <button class="about-license" onclick={onOpenLicense}>GNU AGPL v3</button>
    </div>
    <div class="about-body-clear"></div>
  </div>

  <div class="model-credits-list">
    <div class="credit-item">
      <div class="credit-header">
        <span class="credit-name">LAION CLAP</span>
        <span class="credit-badge badge-apache">Apache 2.0</span>
      </div>
      <p class="credit-desc">Contrastive Language-Audio Pretraining model for audio-text semantic similarity.</p>
      <div class="credit-links">
        <a href="https://huggingface.co/laion/clap-htsat-unfused" target="_blank" rel="noopener noreferrer" class="credit-link">laion/clap-htsat-unfused</a>
        <span class="credit-link-sep">·</span>
        <span class="credit-citation">Wu et al. (ICASSP 2023)</span>
      </div>
    </div>

    <div class="credit-item">
      <div class="credit-header">
        <span class="credit-name">all-MiniLM-L6-v2</span>
        <span class="credit-badge badge-apache">Apache 2.0</span>
      </div>
      <p class="credit-desc">MiniLM sentence transformer model for robust text semantic embeddings.</p>
      <div class="credit-links">
        <a href="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2" target="_blank" rel="noopener noreferrer" class="credit-link">sentence-transformers/all-MiniLM-L6-v2</a>
      </div>
    </div>

    <div class="credit-item">
      <div class="credit-header">
        <span class="credit-name">Essentia Classifiers</span>
        <span class="credit-badge badge-cc">CC BY-NC-ND 4.0</span>
      </div>
      <p class="credit-desc">Discogs-Effnet feature extractor and classifier heads for genre, mood, and vocal classification.</p>
      <div class="credit-links">
        <a href="https://essentia.upf.edu/models.html" target="_blank" rel="noopener noreferrer" class="credit-link">Essentia Models Hub</a>
        <span class="credit-link-sep">·</span>
        <span class="credit-citation">Bogdanov et al. (ISMIR 2013)</span>
      </div>
      <div class="credit-warning">
        Strictly for non-commercial use. Original checkpoints copyright © Music Technology Group (MTG), Universitat Pompeu Fabra.
      </div>
    </div>

    <div class="credit-item">
      <div class="credit-header">
        <span class="credit-name">llama.cpp</span>
        <span class="credit-badge badge-mit">MIT</span>
      </div>
      <p class="credit-desc">LLM inference in C/C++ supporting the local Qwen2-Audio server lifecycle.</p>
      <div class="credit-links">
        <a href="https://github.com/ggerganov/llama.cpp" target="_blank" rel="noopener noreferrer" class="credit-link">ggerganov/llama.cpp</a>
        <span class="credit-link-sep">·</span>
        <span class="credit-citation">Georgi Gerganov</span>
      </div>
    </div>
  </div>
</SettingsCard>

<style>
  :global(.about-card) {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .about-body {
    display: block;
    overflow: hidden;
  }

  .about-body-clear {
    clear: both;
  }

  .about-logo {
    float: right;
    width: 280px;
    height: 280px;
    object-fit: contain;
    opacity: 0.9;
    filter: drop-shadow(0 0 8px color-mix(in srgb, var(--sg-primary) 25%, transparent));
    margin-left: 16px;
    margin-bottom: 8px;
  }

  .about-desc {
    margin: 0;
    font-size: var(--sg-text-sm);
    line-height: 1.65;
    color: var(--sg-on-surface, #e3e1e9);
    opacity: 0.75;
  }

  .about-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-outline, #849495);
  }

  .about-link {
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: color-mix(in srgb, var(--sg-on-surface-muted) 40%, transparent);
    transition: color 0.15s, text-decoration-color 0.15s;
  }
  .about-link:hover {
    color: var(--sg-primary, #00f0ff);
    text-decoration-color: var(--sg-primary, #00f0ff);
  }

  .about-sep { opacity: 0.4; }

  .about-license {
    background: none;
    border: none;
    padding: 0;
    font-family: inherit;
    font-size: inherit;
    color: var(--sg-primary, #00f0ff);
    opacity: 0.75;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: color-mix(in srgb, var(--sg-primary) 40%, transparent);
    transition: opacity 0.15s, text-decoration-color 0.15s;
  }
  .about-license:hover {
    opacity: 1;
    text-decoration-color: var(--sg-primary, #00f0ff);
  }

  .model-credits-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 8px;
    padding-top: 12px;
    border-top: 1px solid color-mix(in srgb, var(--sg-on-surface) 6%, transparent);
    animation: fade-in 0.2s ease-out;
  }

  @keyframes fade-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .credit-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .credit-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .credit-name {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .credit-badge {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    white-space: nowrap;
  }

  .badge-apache {
    border: 1px solid color-mix(in srgb, var(--sg-primary) 30%, transparent);
    color: var(--sg-primary);
    background: color-mix(in srgb, var(--sg-primary) 5%, transparent);
  }

  .badge-cc {
    border: 1px solid rgba(255, 170, 0, 0.3); /* TODO: map to --sg-* token */
    color: #ffaa00; /* TODO: map to --sg-* token */
    background: rgba(255, 170, 0, 0.05); /* TODO: map to --sg-* token */
  }

  .badge-mit {
    border: 1px solid rgba(188, 19, 254, 0.35); /* TODO: map to --sg-* token */
    color: #bc13fe; /* TODO: map to --sg-* token */
    background: rgba(188, 19, 254, 0.05); /* TODO: map to --sg-* token */
  }

  .credit-desc {
    margin: 0;
    font-size: var(--sg-text-xs);
    line-height: 1.4;
    color: var(--sg-on-surface, #e3e1e9);
    opacity: 0.7;
  }

  .credit-links {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
  }

  .credit-link {
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: color-mix(in srgb, var(--sg-on-surface-muted) 30%, transparent);
    transition: color 0.15s, text-decoration-color 0.15s;
  }

  .credit-link:hover {
    color: var(--sg-primary, #00f0ff);
    text-decoration-color: var(--sg-primary, #00f0ff);
  }

  .credit-link-sep {
    opacity: 0.4;
  }

  .credit-citation {
    opacity: 0.8;
  }

  .credit-warning {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: rgba(255, 170, 0, 0.8); /* TODO: map to --sg-* token */
    margin-top: 2px;
    line-height: 1.3;
  }

  .about-license-sep {
    opacity: 0.4;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
  }
</style>
