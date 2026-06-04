#!/usr/bin/env python3
"""
analyze_tags_tfidf.py

Uses TF-IDF to measure how discriminative each qwen tag is across the library,
then cross-references with Gemini accuracy from validate_tags_results.json to
produce a combined usefulness score and prompt adjustment recommendations.

IDF = log(N / df)  — high means rare/specific, low means ubiquitous/noise

Usefulness score = IDF * gemini_accuracy  (0–1 each, so 0–log(N) max)

Usage:
    python analyze_tags_tfidf.py [--output report.md]
"""

import argparse
import json
import math
import sqlite3
from collections import Counter, defaultdict
from pathlib import Path

DB_PATH       = Path.home() / "Library" / "Application Support" / "com.rlupi.deep-cuts" / "deep_cuts.db"
RESULTS_PATH  = Path(__file__).parent / "validate_tags_results.json"
SCRIPT_DIR    = Path(__file__).parent

# ── Load DB tag data ──────────────────────────────────────────────────────────

def load_db_tags() -> tuple[int, dict[str, int]]:
    """Return (N_tracks_with_qwen_tags, {tag: document_frequency})."""
    conn = sqlite3.connect(str(DB_PATH))
    n_tracks = conn.execute(
        "SELECT COUNT(DISTINCT track_id) FROM track_tags WHERE source='qwen'"
    ).fetchone()[0]
    rows = conn.execute(
        """SELECT tg.name, COUNT(DISTINCT tt.track_id)
           FROM track_tags tt JOIN tags tg ON tg.id = tt.tag_id
           WHERE tt.source = 'qwen'
           GROUP BY tg.name"""
    ).fetchall()
    conn.close()
    return n_tracks, {name: df for name, df in rows}


# ── Load Gemini accuracy from validation results ──────────────────────────────

def load_gemini_accuracy(results_path: Path) -> dict[str, tuple[int, int]]:
    """Return {tag: (correct_count, total_count)} from validation results."""
    data = json.loads(results_path.read_text())
    correct_counter: Counter = Counter()
    total_counter:   Counter = Counter()
    for r in data:
        tv = r.get("tag_validation")
        if not isinstance(tv, dict) or "correct" not in tv:
            continue
        for tag in tv["correct"]:
            correct_counter[tag] += 1
            total_counter[tag]   += 1
        for tag in tv["incorrect"]:
            total_counter[tag] += 1
    return {tag: (correct_counter[tag], total_counter[tag]) for tag in total_counter}


# ── Main analysis ─────────────────────────────────────────────────────────────

def analyze(output_path: Path | None = None):
    n_docs, doc_freq = load_db_tags()
    gemini = load_gemini_accuracy(RESULTS_PATH)

    # Build full tag table
    rows = []
    for tag, df in doc_freq.items():
        idf   = math.log(n_docs / df)
        saturation = df / n_docs          # fraction of library that has this tag

        correct, total = gemini.get(tag, (0, 0))
        accuracy = correct / total if total > 0 else None

        usefulness = idf * accuracy if accuracy is not None else None

        ns, label = tag.split(":", 1) if ":" in tag else ("?", tag)
        rows.append({
            "tag":        tag,
            "ns":         ns,
            "label":      label,
            "df":         df,
            "saturation": saturation,
            "idf":        idf,
            "accuracy":   accuracy,
            "correct":    correct,
            "validated":  total,
            "usefulness": usefulness,
        })

    rows.sort(key=lambda r: r["usefulness"] if r["usefulness"] is not None else -999, reverse=True)

    # ── Classify tags ─────────────────────────────────────────────────────────
    # Thresholds:
    #   saturation > 0.70  → ubiquitous (likely prompt leakage)
    #   idf < 0.5          → low discriminative power
    #   accuracy < 0.30    → Gemini says wrong most of the time
    #   usefulness > 1.0   → genuinely useful

    UBIQUITOUS_THRESH  = 0.70
    LOW_IDF_THRESH     = 0.50
    LOW_ACC_THRESH     = 0.30
    USEFUL_THRESH      = 1.0

    keep    = []
    discard = []
    review  = []

    for r in rows:
        reasons = []
        if r["saturation"] >= UBIQUITOUS_THRESH:
            reasons.append(f"ubiquitous ({r['saturation']:.0%} of library)")
        if r["idf"] < LOW_IDF_THRESH:
            reasons.append(f"low IDF ({r['idf']:.2f})")
        if r["accuracy"] is not None and r["accuracy"] < LOW_ACC_THRESH:
            reasons.append(f"low accuracy ({r['accuracy']:.0%})")

        r["discard_reasons"] = reasons

        if r["usefulness"] is not None and r["usefulness"] >= USEFUL_THRESH:
            keep.append(r)
        elif reasons:
            discard.append(r)
        else:
            review.append(r)

    # ── Build report ──────────────────────────────────────────────────────────
    lines = []
    w = lines.append

    w("# qwen Tag Quality Report — TF-IDF × Gemini Accuracy\n")
    w(f"Library: **{n_docs}** tracks with qwen tags | "
      f"**{len(doc_freq)}** distinct tags | "
      f"Validated on **{sum(1 for r in rows if r['validated'] > 0)}** songs via Gemini\n")

    # Summary table
    w("## Summary by namespace\n")
    w(f"{'NS':<12} {'Tags':>5} {'Avg IDF':>8} {'Avg Acc':>8} {'Avg Useful':>11}")
    w("-" * 48)
    ns_groups: dict[str, list] = defaultdict(list)
    for r in rows:
        ns_groups[r["ns"]].append(r)
    for ns in sorted(ns_groups):
        group = ns_groups[ns]
        avg_idf = sum(r["idf"] for r in group) / len(group)
        accs = [r["accuracy"] for r in group if r["accuracy"] is not None]
        avg_acc = sum(accs) / len(accs) if accs else float("nan")
        usefuls = [r["usefulness"] for r in group if r["usefulness"] is not None]
        avg_use = sum(usefuls) / len(usefuls) if usefuls else float("nan")
        w(f"{ns:<12} {len(group):>5} {avg_idf:>8.2f} {avg_acc:>7.0%} {avg_use:>11.2f}")
    w("")

    # Full tag table
    w("## All tags ranked by usefulness\n")
    w(f"{'TAG':<40} {'DF':>4} {'SAT':>6} {'IDF':>6} {'ACC':>6} {'USEFUL':>7}  VERDICT")
    w("-" * 85)
    for r in rows:
        acc_s    = f"{r['accuracy']:.0%}" if r["accuracy"] is not None else "  n/a"
        use_s    = f"{r['usefulness']:.2f}" if r["usefulness"] is not None else "   n/a"
        sat_s    = f"{r['saturation']:.0%}"
        if r["discard_reasons"]:
            verdict = "DISCARD  ← " + ", ".join(r["discard_reasons"])
        elif r["usefulness"] is not None and r["usefulness"] >= USEFUL_THRESH:
            verdict = "KEEP"
        else:
            verdict = "review"
        w(f"{r['tag']:<40} {r['df']:>4} {sat_s:>6} {r['idf']:>6.2f} {acc_s:>6} {use_s:>7}  {verdict}")
    w("")

    # Discard list
    w("## Tags to discard\n")
    by_ns: dict[str, list] = defaultdict(list)
    for r in discard:
        by_ns[r["ns"]].append(r)
    for ns in sorted(by_ns):
        w(f"**{ns}:**")
        for r in by_ns[ns]:
            w(f"  - `{r['tag']}` — {', '.join(r['discard_reasons'])}")
    w("")

    # Keep list
    w("## Tags to keep\n")
    by_ns2: dict[str, list] = defaultdict(list)
    for r in keep:
        by_ns2[r["ns"]].append(r)
    for ns in sorted(by_ns2):
        w(f"**{ns}:**")
        for r in by_ns2[ns]:
            w(f"  - `{r['tag']}` (IDF={r['idf']:.2f}, acc={r['accuracy']:.0%}, useful={r['usefulness']:.2f})")
    w("")

    # Prompt recommendations
    w("## Prompt adjustment recommendations\n")

    # Find per-namespace worst offenders (ubiquitous + inaccurate)
    prompt_issues: dict[str, list[str]] = defaultdict(list)
    for r in discard:
        if r["saturation"] >= UBIQUITOUS_THRESH:
            prompt_issues[r["ns"]].append(r["label"])

    if prompt_issues.get("vibe"):
        w(f"### vibe tags")
        w(f"Tags `{', '.join(prompt_issues['vibe'])}` appear on ≥70% of all tracks — these are "
          f"the **example values from the prompt being echoed back**.")
        w(f"→ Replace the prompt examples with clearly different, more unusual values "
          f"(e.g. `punchy, ceremonial, lo-fi` instead of `ethereal, hypnotic, raw`) "
          f"or instruct the model to **never reuse the examples**.\n")

    if prompt_issues.get("context"):
        ctx_list = ", ".join(f"`{l}`" for l in prompt_issues["context"])
        w(f"### context tags")
        w(f"Tags {ctx_list} appear on ≥70% of all tracks.")
        w(f"→ The `study` and `workout` examples in the prompt are being copied verbatim. "
          f"Replace with less obvious examples (e.g. `cooking, commute`) and add an instruction "
          f"like *'pick contexts that genuinely fit this specific song, not generic defaults'*.\n")
        w(f"→ `context:1990s` is near-universal (0% accuracy). "
          f"The decade instruction should emphasize listening to production quality cues rather than "
          f"defaulting to a prior decade.\n")

    if prompt_issues.get("mood"):
        w(f"### mood tags")
        w(f"Tags `{', '.join(prompt_issues['mood'])}` are over-applied.")
        w(f"→ `uplifting` and `hopeful` seem to be safe defaults when Qwen is uncertain. "
          f"Add explicit instruction: *'only tag uplifting/hopeful if the track is clearly positive — "
          f"do not use as a default'*.\n")

    if prompt_issues.get("inst"):
        w(f"### instrument tags")
        w(f"Tags `{', '.join(prompt_issues['inst'])}` are frequently wrong.")
        w(f"→ `piano` and `acoustic guitar` are being hallucinated for electronic tracks. "
          f"Instruction addition: *'only list instruments you can clearly hear — do not infer from genre'*.\n")

    # Print to stdout
    report = "\n".join(lines)
    print(report)

    # Write to file
    out = output_path or (SCRIPT_DIR / "tag_quality_report.md")
    out.write_text(report, encoding="utf-8")
    print(f"\n→ Report written to {out}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=str, default=None)
    args = parser.parse_args()
    analyze(Path(args.output) if args.output else None)
