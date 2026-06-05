#!/usr/bin/env python3
"""
Replace hardcoded font-family and font-size values with CSS custom property tokens.
Handles both CSS property syntax and JS string literals (D3 .style() calls).
"""
import re
from pathlib import Path

ROOT = Path(__file__).parent.parent / "src"

FONT_FAMILY = [
    (r'"JetBrains Mono",\s*monospace',   'var(--sg-font-mono)'),
    (r"'JetBrains Mono',\s*monospace",   'var(--sg-font-mono)'),
    (r'JetBrains Mono,\s*monospace',     'var(--sg-font-mono)'),
    (r"'Inter',\s*sans-serif",           'var(--sg-font-ui)'),
    (r'"Inter",\s*sans-serif',           'var(--sg-font-ui)'),
    (r'Inter,\s*sans-serif',             'var(--sg-font-ui)'),
    (r"'Outfit',\s*sans-serif",          'var(--sg-font-display)'),
    (r'"Outfit",\s*sans-serif',          'var(--sg-font-display)'),
]

FONT_SIZE_PX = [
    ('8px',  'var(--sg-text-3xs)'),
    ('9px',  'var(--sg-text-2xs)'),
    ('10px', 'var(--sg-text-xs)'),
    ('11px', 'var(--sg-text-sm)'),
    ('12px', 'var(--sg-text-base)'),
    ('14px', 'var(--sg-text-md)'),
]

def replace_css_property(text, prop, pattern, replacement):
    """Replace prop: <pattern> in CSS."""
    return re.sub(
        rf'({re.escape(prop)}\s*:\s*){pattern}',
        rf'\g<1>{replacement}',
        text
    )

def replace_js_style(text, prop, pattern, replacement):
    """Replace .style('prop', '<pattern>') and .style("prop", "<pattern>") in JS."""
    for q in ("'", '"'):
        text = re.sub(
            rf'(\.style\s*\(\s*{q}{re.escape(prop)}{q}\s*,\s*{q}){pattern}({q})',
            rf'\g<1>{replacement}\g<2>',
            text
        )
    return text

def process_file(path: Path) -> tuple[str, int]:
    original = path.read_text(encoding='utf-8')
    text = original

    # font-family replacements
    for pattern, replacement in FONT_FAMILY:
        text = replace_css_property(text, 'font-family', pattern, replacement)
        text = replace_js_style(text, 'font-family', pattern, replacement)

    # font-size replacements — only whole px values, not e.g. 28px, 18px, 38px
    for px, replacement in FONT_SIZE_PX:
        # CSS: font-size: 10px  (must not be preceded by another digit)
        text = re.sub(
            rf'(font-size\s*:\s*)(?<!\d){re.escape(px)}(?!\d)',
            rf'\g<1>{replacement}',
            text
        )
        # JS .style(): .style('font-size', '10px')
        for q in ("'", '"'):
            text = re.sub(
                rf'(\.style\s*\(\s*{q}font-size{q}\s*,\s*{q})(?<!\d){re.escape(px)}(?!\d)({q})',
                rf'\g<1>{replacement}\g<2>',
                text
            )

    changed = sum(1 for a, b in zip(original.splitlines(), text.splitlines()) if a != b)
    return text, changed

def run():
    files = sorted(ROOT.rglob('*.svelte')) + sorted(ROOT.rglob('*.css'))
    total = 0
    for f in files:
        new_text, changes = process_file(f)
        if changes:
            f.write_text(new_text, encoding='utf-8')
            print(f'  {changes:3d} lines  {f.relative_to(ROOT.parent)}')
            total += changes
    print(f'\nTotal lines changed: {total}')

if __name__ == '__main__':
    run()
