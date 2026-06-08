---
name: write-blog-post
description: Draft, structure, and finalize a Deep Cuts blog-series post so it conforms to the post template, house voice, and cross-post link rules. Use when writing, drafting, assembling, or editing a "Deep Cuts" blog post (the rlupi.com series), preparing a post for publishing, or adding front matter to one. Covers the template, front matter schema, voice, agent crediting, and the link linter.
---

# Writing a Deep Cuts Blog Post

The Deep Cuts blog series is a first-person, build-in-public chronicle published at `rlupi.com`. Sources live in the **private** repo at `doc/private/` (its own git repo): drafts as `blog_draft_*.md`, published sources as `blog_post_*.md`, plans as `blog_plan_*.md`. The canonical structure is `doc/private/blog_template.md` — **read it first and mirror it.** (Roberto reviews and adjusts the template; treat the live file as source of truth, not this skill's summary.)

This skill is the *judgment* half of conformance — structure and voice. The *mechanical* half (links) is enforced by `tools/lint_blog_links.py`; run it before handing a post over.

## Front matter (every post)

Start each post with the YAML block from the template. The link linter requires three fields; keep them accurate:

- `slug` — the `rlupi.com/<slug>`. Hashnode derives it from the title (lowercase, punctuation → hyphens); set it explicitly so cross-post links resolve.
- `status` — `draft` until it is live, then `published`.
- `published_url` — filled in once live.

Also set `title`, `series_post` (the "post N" number), and `date`.

## Structure

Mirror the template and the existing posts:

1. `# Title`, then the byline `*Deep Cuts — post N*`, then `---`.
2. **Intro** — open with a callback to the previous post (see link rule below), then state the thesis in one or two narrow, honest sentences. Not "look how fast AI is."
3. `##` sections separated by `---`. Keep them concrete: real commits, timestamps, tool output, code. Show, don't claim.
4. **An honest-counterweight section** is required — where it doesn't work, what it cost, what broke. The series oversells nothing, including its own process.
5. **"Where this leaves us"** close, ending on a one-paragraph teaser for the next post (the series' signature forward hook).

## Cross-post links — public URLs only

**Never** link to another post with a repo path like `./blog_post_xyz.md`. It works in the repo and 404s once published. Always link to the target's public URL: `https://rlupi.com/<target-slug>`, taken from that post's `slug` front matter, and only if its `status` is `published`. Before finishing, run:

```bash
python3 tools/lint_blog_links.py --dir doc/private          # report
python3 tools/lint_blog_links.py --dir doc/private --fix    # rewrite relative -> public
```

To hand Roberto a paste-ready copy plus the list of images to upload:

```bash
python3 tools/lint_blog_links.py --export doc/private/<post>.md
```

## Credit every agent

When the work was collaborative, credit each contributing agent by name and lab — claude/Anthropic, agy=Gemini/Google, codex/OpenAI, meta/Meta — not just the assembler. The cross-company collaboration is the story, not a footnote. If you quote another agent's text, frame it in one line and keep it verbatim. (See the agent roster in memory.)

## Images

Reference figures with standard markdown (`![Caption](./blog_figN_name.png)`). Images are uploaded by hand at publish time; `--export` lists every image a post references so the files are ready. Posting is manual — do not attempt to publish programmatically.
