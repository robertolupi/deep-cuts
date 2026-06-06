---
name: ui-design
description: Guidelines for the Sonic Glitch design system, theme variables, and multi-theme readability (dark, light, high-contrast)
---

# UI Design & Theme Guidelines

This document outlines the design conventions for the Deep Cuts desktop application. All frontend components must adhere to these guidelines to ensure consistent visual aesthetics, clean code, and accessibility across all themes.

## 1. The Sonic Glitch Design System

Deep Cuts uses custom CSS variables starting with `--sg-` for layout, colors, effects, and typography. **Never hardcode hex, RGB, RGBA, or HSL values** directly in components. Always use the design system tokens or derive from them with `color-mix()`.

### Core Theme Colors
- **App Shell & Panel Surfaces**: `var(--sg-surface)`, `var(--sg-surface-dim)`, `var(--sg-surface-low)`, `var(--sg-surface-container)`, `var(--sg-surface-slate)`.
- **Text & Foreground Elements**:
  - `var(--sg-on-surface)`: Primary text / high emphasis.
  - `var(--sg-on-surface-variant)`: Secondary text / medium emphasis.
  - `var(--sg-on-surface-muted)`: Timestamps, captions, low emphasis.
- **Accents**:
  - `var(--sg-primary)`: Cyber Cyan (used for active interactive states, filters).
  - `var(--sg-secondary)`: Studio Pink (used for AI-generated metadata, vibes, descriptions).
- **Semantics**:
  - `var(--sg-error)`: Red (errors, warnings, silence flags).
  - `var(--sg-success)`: Green/teal (success indicators).
  - `var(--sg-warning)`: Yellow/amber (warnings, notice highlights).

---

## 2. Multi-Theme Adaptability

The app supports three themes (Dark, Light, and Accessible High Contrast) configured via the `html[data-theme]` attribute. When implementing UI features, ensure they render beautifully and legibility is maintained under each:

### Dark Theme (`html[data-theme="dark"]` - Default)
- Neon glows and cyberpunk aesthetics are permitted.
- Uses dark gray/slate surfaces with bright cybercyan and pink highlights.

### Light Theme (`html[data-theme="light"]`)
- Ambient glow is suppressed (`--sg-ambient-bg: none`).
- Accents are shifted to desaturated, AA-safe color spectrums (e.g., deep teal instead of neon cyan, muted plum instead of neon magenta).
- Text contrast must meet a minimum of **4.5:1** for normal text and **3:1** for large text against their respective panel backgrounds.

### Accessible Theme (`html[data-theme="accessible"]`)
- Strict high contrast (pure black `#000000` background, pure white `#ffffff` or near-white text).
- Glassmorphism, background blurs, drop shadows, and radial gradients are disabled.
- Motion and transitions are disabled (`--sg-transition: none`).
- All interactive controls (buttons, inputs, dropdowns) must use solid high-contrast borders and sharp corners.
- Interactive elements must use foreground/background colors that yield high legibility (e.g. black text on a cyan button).

---

## 3. Creating Custom Styles and Classes

If you must define new component-specific styling (such as badges, specialized borders, or custom list rows):

1. **Map to Existing Semantics**:
   For example, if you are styling a "Silence" or "Broken file" warning badge, do not use `color: red;` or a hardcoded hex color. Instead, use:
   ```css
   .silence-badge {
     color: var(--sg-error);
     background: color-mix(in srgb, var(--sg-error) 8%, transparent);
     border: 1px solid color-mix(in srgb, var(--sg-error) 30%, transparent);
   }
   ```
2. **Support the Accessible High-Contrast Theme**:
   Always add overrides in your CSS for the accessible theme to ensure readability. For example:
   ```css
   html[data-theme="accessible"] .silence-badge {
     border-radius: 0;
     border: 2px solid var(--sg-on-surface) !important;
     background: var(--sg-surface) !important;
     color: var(--sg-on-surface) !important;
   }
   ```
3. **Typography Standards**:
   - UI elements and tables must use `var(--sg-font-ui)` (Inter / system font).
   - Technical values, keys, and paths must use `var(--sg-font-mono)` (JetBrains Mono).

## 4. Verification Checklist

Before finishing UI work:

- Search changed components for hardcoded `#`, `rgb(`, `rgba(`, and inline `style="color`.
- Verify dark, light, and accessible themes.
- For canvas/SVG palettes, read colors from CSS variables via `getComputedStyle()` instead of duplicating literals in TypeScript.
- Confirm focus, hover, active, disabled, loading, empty, and error states are legible.
- If layout or styling changed materially, follow `skills/ui-debug/SKILL.md` for browser verification.
