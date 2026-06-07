# Approach A: DTW Block Query Search & Composer (Tabled State)

**Status:** Tabled  
**Date:** 2026-06-06  

This document preserves the design and state of Approach A for when we return to it.

## Core Design

The goal of Approach A is to provide a visual block query composer where users can sketch a structural song arc (e.g. `[Intro] -> [Build] -> [Chorus]`) and match it against the 32-character SAX envelope string (`waveform_sax`) of all library tracks using Dynamic Time Warping (DTW).

### 1. Contextual Block-to-Sequence Compilation

Instead of matching single characters, blocks compile into multi-character sequences representing context and transitions:

| Block Type | Target SAX Letters | Notes |
|---|---|---|
| `[Intro]` | `a a a` | Low energy, must be at track start |
| `[Outro]` | `b b b` | Low-mid energy, must be at track end |
| `[Verse]` | `c c c` | Moderate energy |
| `[Chorus]` | `e e e` | High energy peak |
| `[Pre-Chorus]` | `c d` | Transition |
| `[Bridge]` | `d d` | Mid-high energy |
| `[Break]` | `d b d` | Quiet segment surrounded by louder ones |
| `[Build]` | `a b c d e` | Monotonic rising ramp |
| `[Drop]` | `e b e` | Loud -> sudden drop -> loud |
| `[···]` | `*` | Wildcard (matches any character at 0 cost) |

### 2. Anchoring Rules

* If the first block is **not** `[Intro]`, prepend an implicit wildcard `*` to allow free prefix matching.
* If the last block is **not** `[Outro]`, append an implicit wildcard `*` to allow free suffix matching.

### 3. DTW Alignment with Wildcards

A fast client-side DTW implementation computes the distance between the compiled target sequence $Q$ (length $N$) and the track's 32-character SAX string $T$ (length $M$):
- If $Q[i-1] == '*'$ (wildcard), alignment cost is 0.
- Otherwise, cost is normalized: `abs(Q[i-1] - T[j-1]) / 4.0`.

### 4. Performance & Pre-Filtering

To prevent UI lag when searching a large library:
- Only compute DTW scores for tracks that pass the active textual/metadata/vibe filters.
- Debounce structural query changes by 250ms when no other filters are active.
