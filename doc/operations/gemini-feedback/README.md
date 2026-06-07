# Gemini Feedback Index

Date: 2026-06-07
Author: Gemini 3.5 Flash (Medium)

This directory collects the codebase, architecture, and development workflow review performed by Gemini.

## Files

* [project-opinion-swot.md](project-opinion-swot.md) — Overall project opinion, SWOT analysis, and recommended product spine.
* [technical-observations.md](technical-observations.md) — Detailed technical breakdown of database serialization, component decomposition, model UX, and test coverage gaps.

## Summary of Core Findings

1. **Machine-Driven Velocity & Debt Accrual:** With 457 commits in 9 days (~50 commits/day), the project has matured at a rapid pace. The risk is that architectural debt (bloated Svelte components, brittle query DTOs) is also accumulating at machine-assisted speed.
2. **Brittle Data Serialization:** The use of raw positional indices (`row.get(index)`) across 60+ fields in the backend represents the highest structural risk to schema stability. Named query maps or automatic serde mappers should be adopted.
3. **UI Component Decomposition:** Front-end components exceed 1,200 lines and combine DOM structure, visual styles, and data algorithms. Splitting these into sub-components and pure utility helpers is critical for long-term maintainability.
4. **Progressive Degradation for Models:** Requiring a 6 GB Hugging Face download (mostly the Qwen audio LLM) creates a huge setup barrier. The app should degrade gracefully if certain models are absent (e.g., allowing local DSP analysis, filtering, and UMAP coordinates even without the LLM chat feature).
5. **SAX Structural Search is the Core Wedge:** Symbolic Aggregate Approximation (SAX) structural matching and Viterbi sequence alignment are highly novel, low-compute features that mainstream tools do not expose. This should be treated as the marquee product differentiator.
