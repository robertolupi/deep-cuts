#!/usr/bin/env python3
"""Link linter for Deep Cuts blog posts (doc/private).

Mechanical checks only — this is NOT a prose/format linter. Conformance to
blog_template.md (structure, voice, the honest-counterweight beat) is a judgment
task handled by the `write-blog-post` skill, not by code.

What it checks:
  * No post links to another post with a repo-relative path (`./blog_xyz.md`).
    Those resolve in the repo and 404 once published to rlupi.com.
  * Every cross-post link points at the target's public URL, derived from the
    target's `slug` front matter (`https://rlupi.com/<slug>`).
  * No post links to a target whose `status` is not `published` (also a 404).

Usage:
  lint_blog_links.py [--dir DIR]            # report problems, exit 1 if any
  lint_blog_links.py [--dir DIR] --fix      # rewrite relative cross-post links
  lint_blog_links.py [--dir DIR] --check-live   # HTTP-check referenced rlupi URLs
  lint_blog_links.py --export POST.md       # paste-ready text (no front matter,
                                            #   links fixed) + image manifest
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

SITE = "https://rlupi.com"
# A post source file: blog_post_*.md or blog_draft_post*.md, but not a fragment
# (*_section.md) and not the template.
POST_RE = re.compile(r"^blog_(post_|draft_post).*\.md$")
# Markdown link to a repo-relative blog file: [text](./blog_xyz.md#frag)
REL_LINK_RE = re.compile(r"\]\(\s*\.?/?(blog_[A-Za-z0-9_]+\.md)([^)\s]*)\s*\)")
# Markdown link to a public post URL: [text](https://rlupi.com/slug)
SITE_LINK_RE = re.compile(r"\]\(\s*" + re.escape(SITE) + r"/([a-z0-9-]+)[^)]*\)")
IMAGE_RE = re.compile(r"!\[[^\]]*\]\(\s*([^)\s]+)")


def is_post(path: Path) -> bool:
    return bool(POST_RE.match(path.name)) and not path.name.endswith("_section.md")


def parse_front_matter(text: str) -> tuple[dict, str]:
    """Return (flat-scalar front matter dict, body). Nested keys (e.g. tags) are
    recorded as present with an empty value — the linter only needs scalars."""
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        return {}, text
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            fm_lines, body = lines[1:i], "\n".join(lines[i + 1 :])
            break
    else:
        return {}, text
    fm: dict[str, str] = {}
    for line in fm_lines:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line[0] in " \t-":  # nested list / mapping item; skip detail
            continue
        if ":" in line:
            key, _, val = line.partition(":")
            fm[key.strip()] = val.strip()
    return fm, body


def load_posts(blog_dir: Path) -> dict[str, dict]:
    """filename -> {slug, status, title, path}."""
    posts: dict[str, dict] = {}
    for p in sorted(blog_dir.glob("*.md")):
        if not is_post(p):
            continue
        fm, body = parse_front_matter(p.read_text())
        title = fm.get("title")
        if not title:
            m = re.search(r"^#\s+(.+)$", body, re.MULTILINE)
            title = m.group(1).strip() if m else None
        posts[p.name] = {
            "slug": fm.get("slug"),
            "status": fm.get("status"),
            "title": title,
            "path": p,
        }
    return posts


def public_url(slug: str) -> str:
    return f"{SITE}/{slug}"


def lint(blog_dir: Path, fix: bool) -> tuple[list[str], list[str]]:
    posts = load_posts(blog_dir)
    errors: list[str] = []
    warnings: list[str] = []

    for name, meta in posts.items():
        path: Path = meta["path"]
        text = path.read_text()
        _, body = parse_front_matter(text)

        # Conformance is a skill's job, but the *link* checks need slug/status,
        # so flag posts missing those two fields (they break link resolution).
        if not meta["slug"]:
            warnings.append(f"{name}: missing `slug` front matter (links to it can't resolve)")
        if not meta["status"]:
            warnings.append(f"{name}: missing `status` front matter")

        for m in REL_LINK_RE.finditer(body):
            target_file, frag = m.group(1), m.group(2)
            tgt = posts.get(target_file)
            if tgt is None:
                errors.append(
                    f"{name}: links to `{target_file}` via repo path, and no such "
                    f"post/front matter exists to resolve a public URL"
                )
                continue
            if not tgt["slug"]:
                errors.append(
                    f"{name}: links to `{target_file}` via repo path; that post has "
                    f"no `slug` front matter to rewrite to"
                )
                continue
            if tgt["status"] != "published":
                errors.append(
                    f"{name}: links to `{target_file}` (status={tgt['status']!r}); "
                    f"that post is not published — would 404"
                )
                continue
            errors.append(
                f"{name}: repo-relative link `./{target_file}{frag}` -> should be "
                f"`{public_url(tgt['slug'])}{frag}`"
            )
        if fix:
            fixed = REL_LINK_RE.sub(lambda m: _fix_rel(m, posts), text)
            if fixed != text:
                path.write_text(fixed)

    return errors, warnings


def _fix_rel(m: re.Match, posts: dict) -> str:
    target_file, frag = m.group(1), m.group(2)
    tgt = posts.get(target_file)
    if tgt and tgt.get("slug") and tgt.get("status") == "published":
        return f"]({public_url(tgt['slug'])}{frag})"
    return m.group(0)  # leave unresolvable links untouched (already an error)


def check_live(blog_dir: Path) -> list[str]:
    import urllib.request

    posts = load_posts(blog_dir)
    urls: set[str] = set()
    for meta in posts.values():
        text = meta["path"].read_text()
        for m in SITE_LINK_RE.finditer(text):
            urls.add(public_url(m.group(1)))
    problems = []
    for url in sorted(urls):
        req = urllib.request.Request(url, method="HEAD", headers={"User-Agent": "deep-cuts-link-linter"})
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                if resp.status >= 400:
                    problems.append(f"{url} -> HTTP {resp.status}")
        except urllib.error.HTTPError as e:
            if e.code == 429:  # rate-limited: can't confirm, don't cry wolf
                problems.append(f"{url} -> 429 rate-limited (could not verify)")
            else:
                problems.append(f"{url} -> HTTP {e.code}")
        except Exception as e:  # noqa: BLE001
            problems.append(f"{url} -> {e}")
    return problems


def export(post: Path, blog_dir: Path) -> None:
    posts = load_posts(blog_dir)
    text = post.read_text()
    text = REL_LINK_RE.sub(lambda m: _fix_rel(m, posts), text)
    _, body = parse_front_matter(text)
    sys.stdout.write(body.lstrip("\n"))
    images = IMAGE_RE.findall(body)
    if images:
        sys.stderr.write("\n--- images referenced (upload these on publish) ---\n")
        for ref in images:
            if ref.startswith(("http://", "https://")):
                sys.stderr.write(f"  remote : {ref}\n")
            else:
                local = (post.parent / ref).resolve()
                mark = "ok" if local.exists() else "MISSING"
                sys.stderr.write(f"  {mark:7}: {local}\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dir", default="doc/private", help="blog directory (default: doc/private)")
    ap.add_argument("--fix", action="store_true", help="rewrite repo-relative cross-post links to public URLs")
    ap.add_argument("--check-live", action="store_true", help="HTTP-check referenced rlupi.com URLs")
    ap.add_argument("--export", metavar="POST.md", help="emit paste-ready text + image manifest for one post")
    args = ap.parse_args()

    blog_dir = Path(args.dir)
    if args.export:
        export(Path(args.export), blog_dir)
        return 0
    if not blog_dir.is_dir():
        print(f"error: blog dir not found: {blog_dir}", file=sys.stderr)
        return 2

    errors, warnings = lint(blog_dir, args.fix)
    if args.fix:
        errors, warnings = lint(blog_dir, False)  # re-check after fixing

    for w in warnings:
        print(f"warning: {w}")
    for e in errors:
        print(f"error: {e}")

    if args.check_live:
        for p in check_live(blog_dir):
            print(f"dead-link: {p}")

    if errors:
        print(f"\n{len(errors)} link error(s)." + ("" if args.fix else " Run with --fix to rewrite."))
        return 1
    print("blog links OK." if not warnings else f"\nlinks OK, {len(warnings)} warning(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
