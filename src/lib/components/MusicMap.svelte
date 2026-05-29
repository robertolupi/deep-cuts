<section class="map-panel glass-panel">
  <h2>The Music Map</h2>
  <p class="map-desc">Your CLAP audio embeddings will be projected in 2D space here using UMAP dimensionality reduction.</p>
  
  <div class="map-canvas-mockup">
    <div class="mock-dots">
      {#each Array(50) as _, i}
        <div 
          class="mock-dot" 
          style="
            left: {20 + (i * 1.5 + Math.sin(i) * 15) % 65}%; 
            top: {15 + (i * 2 + Math.cos(i) * 12) % 70}%; 
            background-color: {i % 3 === 0 ? 'var(--color-accent-cyan)' : i % 3 === 1 ? 'var(--color-accent-magenta)' : 'var(--color-primary)'}
          "
        ></div>
      {/each}
    </div>
    <p class="map-overlay-text">Visual Map Skeleton Ready</p>
  </div>
</section>
