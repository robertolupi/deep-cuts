#!/usr/bin/env python3
"""
CSS audit: extracts hardcoded font-size, font-family, border, border-radius,
gap, padding, margin, width, height values from Svelte <style> blocks and CSS files.
Outputs JSON grouped by file → rule selector → properties.

Usage: python scripts/css-audit.py [--out audit.json]
"""

import re
import json
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent / "src"

# Properties to capture when they contain a hardcoded (non-var()) value
TARGET_PROPS = {
    "font-size", "font-family", "font-weight",
    "border", "border-top", "border-right", "border-bottom", "border-left",
    "border-width", "border-radius",
    "gap", "row-gap", "column-gap",
    "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
    "margin", "margin-top", "margin-right", "margin-bottom", "margin-left",
    "width", "height", "min-width", "max-width", "min-height", "max-height",
    "box-shadow", "letter-spacing", "line-height",
}

# A value is "hardcoded" if it contains a px/em/rem/% literal and no var()
def is_hardcoded(value: str) -> bool:
    v = value.strip()
    if v.startswith("var("):
        return False
    return bool(re.search(r"\d+(\.\d+)?(px|em|rem|%|pt)", v))

def extract_css_blocks(path: Path) -> list[str]:
    """Return CSS text to parse: full file for .css, <style> blocks for .svelte."""
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".css":
        return [text]
    blocks = re.findall(r"<style[^>]*>(.*?)</style>", text, re.DOTALL)
    return blocks

def parse_rules(css: str) -> list[dict]:
    """
    Parse CSS into a flat list of {selector, properties}.
    Handles nested at-rules by unwrapping one level.
    """
    # Strip comments
    css = re.sub(r"/\*.*?\*/", "", css, flags=re.DOTALL)

    results = []

    def parse_block(text: str, context: str = ""):
        # Split into top-level {...} blocks
        depth = 0
        start = 0
        selector_start = 0
        i = 0
        while i < len(text):
            ch = text[i]
            if ch == "{":
                if depth == 0:
                    raw_selector = text[selector_start:i].strip()
                    block_start = i + 1
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    block = text[block_start:i]
                    raw_selector = text[selector_start:block_start - 1].strip()
                    selector_start = i + 1

                    # At-rule? unwrap and recurse
                    if raw_selector.startswith("@"):
                        parse_block(block, context=raw_selector)
                    else:
                        # Actual rule — extract target properties
                        props = {}
                        for decl in block.split(";"):
                            m = re.match(r"\s*([\w-]+)\s*:\s*(.+)", decl.strip())
                            if not m:
                                continue
                            prop, val = m.group(1).lower(), m.group(2).strip()
                            if prop in TARGET_PROPS and is_hardcoded(val):
                                props[prop] = val
                        if props:
                            sel = raw_selector.replace("\n", " ")
                            sel = re.sub(r"\s+", " ", sel).strip()
                            # Strip Svelte scoping hashes for readability
                            sel = re.sub(r"\.s-[A-Za-z0-9_]+", "", sel).strip()
                            entry = {"selector": sel, "properties": props}
                            if context:
                                entry["context"] = context
                            results.append(entry)
            i += 1

    parse_block(css)
    return results

def relative(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT.parent))
    except ValueError:
        return str(path)

def run(out_path: Path | None = None):
    files = sorted(ROOT.rglob("*.svelte")) + sorted(ROOT.rglob("*.css"))
    audit = {}

    for f in files:
        rel = relative(f)
        blocks = extract_css_blocks(f)
        rules = []
        for block in blocks:
            rules.extend(parse_rules(block))
        if rules:
            audit[rel] = rules

    # Summary stats
    total_rules = sum(len(v) for v in audit.values())
    prop_counts: dict[str, int] = {}
    for rules in audit.values():
        for rule in rules:
            for prop in rule["properties"]:
                prop_counts[prop] = prop_counts.get(prop, 0) + 1

    output = {
        "summary": {
            "files_with_hardcoded_values": len(audit),
            "total_rules": total_rules,
            "property_frequency": dict(sorted(prop_counts.items(), key=lambda x: -x[1])),
        },
        "files": audit,
    }

    result = json.dumps(output, indent=2)

    if out_path:
        out_path.write_text(result, encoding="utf-8")
        print(f"Written to {out_path}  ({total_rules} rules across {len(audit)} files)")
    else:
        print(result)

if __name__ == "__main__":
    out = None
    if "--out" in sys.argv:
        idx = sys.argv.index("--out")
        out = Path(sys.argv[idx + 1])
    run(out)
