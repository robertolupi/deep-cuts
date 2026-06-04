# Library Maintenance & Validation Utilities

This document proposes catalog verification tools to audit, detect, and flag broken or silent files in the library.

---

## 1. Silent & File-Integrity Detector
Detect tracks containing unusually long silence segments or files with potential formatting issues:
- **How it works**: Queries the `has_long_silence` and `silence_regions` columns populated during the audio-analysis pass.
- **Implementation**: Exposes a filter chip in the sidebar labeled `[Tracks with Long Silences]`. Rather than marking files as broken automatically (since classical tracks can have legitimate pianissimo sections), it highlights tracks with silences exceeding a threshold for user audit.

---

## 2. Low-Audio & Dynamic Range Auditing (Waveform + LUFS)
By combining the relative 128-point envelope with the absolute `loudness_lufs` metric, we can analyze the dynamic profile of tracks:
- **Dynamic Range Mapping (Brickwall Detector)**: Standardize the 128-point envelope vector using the integrated absolute LUFS value. Tracks with very low variance (a flat, maximized envelope near -6 LUFS) are flagged as "brickwalled" (highly compressed/limited masters). Tracks with high variance are flagged as "dynamic" (classical, jazz, acoustic).
  * *Tagging Integration*: To prevent the filter sidebar from becoming unwieldy, these mastering profiles (e.g., `mastering:brickwalled` vs. `mastering:dynamic` or `mastering:sober`) can be auto-generated and applied as system tags when the generalized tagging framework is implemented.
- **Absolute Gain/Breakdown Warnings**: Identify tracks whose breakdowns drop to extreme absolute quiet (e.g. below -24 LUFS) to alert the user of dramatic level changes during mixing, or calculate optimal gain offsets for sections rather than relying on a global track average.
