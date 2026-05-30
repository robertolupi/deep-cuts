---
name: Sonic Glitch
colors:
  surface: '#121318'
  surface-dim: '#121318'
  surface-bright: '#38393f'
  surface-container-lowest: '#0d0e13'
  surface-container-low: '#1a1b21'
  surface-container: '#1e1f25'
  surface-container-high: '#292a2f'
  surface-container-highest: '#34343a'
  on-surface: '#e3e1e9'
  on-surface-variant: '#b9cacb'
  inverse-surface: '#e3e1e9'
  inverse-on-surface: '#2f3036'
  outline: '#849495'
  outline-variant: '#3b494b'
  surface-tint: '#00dbe9'
  primary: '#dbfcff'
  on-primary: '#00363a'
  primary-container: '#00f0ff'
  on-primary-container: '#006970'
  inverse-primary: '#006970'
  secondary: '#ffabf3'
  on-secondary: '#5b005b'
  secondary-container: '#fe00fe'
  on-secondary-container: '#500050'
  tertiary: '#faf3ff'
  on-tertiary: '#3c0090'
  tertiary-container: '#e1d2ff'
  on-tertiary-container: '#7213ff'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#7df4ff'
  primary-fixed-dim: '#00dbe9'
  on-primary-fixed: '#002022'
  on-primary-fixed-variant: '#004f54'
  secondary-fixed: '#ffd7f5'
  secondary-fixed-dim: '#ffabf3'
  on-secondary-fixed: '#380038'
  on-secondary-fixed-variant: '#810081'
  tertiary-fixed: '#e9ddff'
  tertiary-fixed-dim: '#d1bcff'
  on-tertiary-fixed: '#23005b'
  on-tertiary-fixed-variant: '#5700c9'
  background: '#121318'
  on-background: '#e3e1e9'
  surface-variant: '#34343a'
  surface-slate: '#161B22'
  border-glass: rgba(255, 255, 255, 0.08)
  glow-cyan: rgba(0, 240, 255, 0.4)
  glow-pink: rgba(255, 0, 255, 0.3)
  waveform-bg: '#0D1117'
typography:
  display-lg:
    fontFamily: Inter
    fontSize: 48px
    fontWeight: '700'
    lineHeight: '1.1'
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '600'
    lineHeight: '1.2'
  body-base:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '400'
    lineHeight: '1.5'
  meta-mono:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '500'
    lineHeight: '1.4'
    letterSpacing: 0.02em
  label-caps:
    fontFamily: JetBrains Mono
    fontSize: 10px
    fontWeight: '700'
    lineHeight: '1'
    letterSpacing: 0.1em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  sidebar-width: 260px
  detail-pane-width: 320px
  player-bar-height: 80px
  gap-sm: 8px
  gap-md: 16px
  container-padding: 24px
---

## Brand & Style

The design system establishes a **Cyberpunk Glassmorphism** aesthetic tailored for high-precision audio analysis. It targets a "Prosumer" audience—audiophiles, DJs, and producers—who require studio-grade data presented through a cutting-edge, futuristic lens. 

The style utilizes deep, atmospheric backgrounds to create a sense of focused immersion, while vibrant accents evoke the glowing filaments of a high-end rack processor. UI elements are treated as semi-transparent "glass" surfaces with light-refracting blurs, creating physical depth and organizational hierarchy without sacrificing the "always-on" data density required for professional workflows.

**Keywords:** Kinetic, Precise, Neon-Atmospheric, Transparent, Technical.

## Colors

The palette is anchored by **Deep Indigo** and **Dark Slate** neutrals to provide a stable, low-strain background for long analysis sessions. 

- **Cyber-Cyan (Primary):** Used for active states, playback progress, and primary actions. It represents the "High-Tech" soul of the app.
- **Studio-Pink (Secondary):** Used for secondary highlights, AI-generated insights, and "Feels" metadata.
- **Vibrant Violet (Tertiary):** Used for deep-level categorization and complex UMAP groupings.

**Color Application:**
- Backgrounds use the neutral base with a 2% blue tint to prevent "pure black" flatness.
- Interactive elements utilize a "glow-state" on hover, using `glow-cyan` or `glow-pink` as a diffused outer shadow.
- Glass panels should use a background color of `rgba(22, 27, 34, 0.7)` combined with a 20px backdrop filter blur.

## Typography

The system employs a dual-font strategy:
1. **Inter:** Handles all UI chrome and prose. Its neutral, high-legibility structure balances the visual intensity of the glassmorphic style.
2. **JetBrains Mono:** Reserved for technical metadata (BPM, Key, Sample Rate, File Paths). The monospaced nature emphasizes the precision of the audio analysis engine.

**Scaling Rules:**
- Headlines on mobile should scale down by 20% to prevent overflow in narrow panes.
- Monospace labels should never drop below 10px to ensure legibility on high-DPI displays.
- All technical data in tables must use tabular numbers (built into JetBrains Mono) to ensure vertical alignment of digits.

## Layout & Spacing

The layout follows a **structured multi-pane model** optimized for widescreen desktop use. 

- **The Core:** A three-column structure consisting of a collapsible **Filter Sidebar** (Left), a fluid **Main Content Area** (Center), and a collapsible **Track Detail Pane** (Right).
- **The Anchor:** A persistent **Player Bar** at the bottom provides a constant reference point for playback state.
- **Grids:** Internal modules use an 8px base grid.
- **Responsiveness:** As the window shrinks, priority is given to the Main Content Area. The Detail Pane is the first to collapse into an overlay, followed by the Filter Sidebar. On mobile/compact views, the layout pivots to a single-pane view with a bottom-sheet for the player.

## Elevation & Depth

Hierarchy is defined through **Glassmorphism and Tonal Layering** rather than traditional drop shadows.

1.  **Level 0 (Base):** Deep Slate background.
2.  **Level 1 (Panels):** Semi-transparent `surface-slate` with a `border-glass` (1px solid). Includes a `backdrop-filter: blur(20px)`.
3.  **Level 2 (Active/Floating):** Higher transparency with a subtle `glow-cyan` inner border (0.5px) to indicate focus or active selection.
4.  **Audio Surfaces:** The waveform and spectrogram areas are "recessed" using a darker, opaque `waveform-bg` to distinguish them from the glass UI panels.

## Shapes

The design system uses a **Soft (0.25rem)** roundedness approach. This provides a modern feel that isn't overly organic or "bubbly," maintaining the professional, technical tone of a studio tool.

- **Standard Buttons & Inputs:** 4px radius.
- **Major Panels:** 8px radius (`rounded-lg`) to soften the large layout blocks.
- **Interactive Tags/Chips:** Pill-shaped (fully rounded) to distinguish them from structural elements.
- **Dividers:** Fine, 1px lines with a 50% opacity gradient to separate data points without creating visual clutter.

## Components

### Buttons
- **Primary:** Gradient background (`primary` to `tertiary`), white text, subtle cyan outer glow on hover.
- **Ghost/Glass:** `border-glass` with a blur background. On hover, the border opacity increases.
- **Transport Controls:** Circular buttons with glowing icon states (Cyan for Play, Gray for Pause/Stop).

### Inputs & Filters
- **Search Fields:** Integrated magnifying glass icon, `meta-mono` placeholder text, and a glowing bottom-border on focus.
- **Range Sliders:** Dual-handle cyan sliders with monospaced value callouts that appear only during interaction.
- **Genre Chips:** Dark semi-transparent backgrounds that turn vibrant Cyan or Pink when toggled "on."

### Cards & Lists
- **Table Rows:** Hover state triggers a subtle 5% lighten effect. Active/Playing row receives a `primary` left-border accent.
- **Waveform Thumbnails:** Rendered in a monochrome gray by default, turning into a Cyan/Pink gradient when the track is selected.

### Track Detail Pane
- **Mood Bars:** Thin horizontal bars that fill based on percentage. Use a "charging" animation when the track is first loaded.
- **Metadata Grid:** Labels in `label-caps` (Gray) and values in `meta-mono` (White).

### Player Bar
- **Spectrogram:** Hidden by default. When toggled, the bar expands vertically with a spring animation.
- **Progress:** A high-contrast Cyan line that leaves a subtle trail/glow as it moves across the waveform.