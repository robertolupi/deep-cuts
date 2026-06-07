---
status: proposed
owner: Roberto
last_verified: 2026-06-07
implemented_by:
superseded_by:
related_code:
related_skills:
---

# Library Maintenance & Validation Utilities

This document proposes catalog verification tools to audit and flag files in the library.

---

## 1. Low-Audio & Dynamic Range Auditing (Waveform + LUFS)
By combining the relative 128-point envelope with the absolute `loudness_lufs` metric, we can analyze the dynamic profile of tracks:
- **Dynamic Range Mapping (Brickwall Detector)**: Standardize the 128-point envelope vector using the integrated absolute LUFS value. Tracks with very low variance (a flat, maximized envelope near -6 LUFS) are flagged as "brickwalled" (highly compressed/limited masters). Tracks with high variance are flagged as "dynamic" (classical, jazz, acoustic).
  * *Tagging Integration*: To prevent the filter sidebar from becoming unwieldy, these mastering profiles (e.g., `mastering:brickwalled` vs. `mastering:dynamic` or `mastering:sober`) can be auto-generated and applied as system tags when the generalized tagging framework is implemented.
- **Absolute Gain/Breakdown Warnings**: Identify tracks whose breakdowns drop to extreme absolute quiet (e.g. below -24 LUFS) to alert the user of dramatic level changes during mixing, or calculate optimal gain offsets for sections rather than relying on a global track average.
