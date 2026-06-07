#!/usr/bin/env python3
"""
Deep Cuts Collab Hub — a THIN viewer + human-gated launcher over the markdown
collaboration protocol. Run with:  streamlit run tools/collab_hub.py

SAFETY INVARIANTS (do not remove):
  1. NEVER pass --dangerously-skip-permissions. Agents run under the repo's
     .claude/settings.json allowlist, and this hub additionally blocks `Bash`
     for invoked agents — so an invoked agent cannot shell out to spawn a peer.
     No Bash -> no peer invocation -> no Claude<->Gemini runaway loop.
  2. One click = one turn = stop. Nothing auto-advances the turn. The human is
     the clock; an agent's reply NEVER auto-triggers another agent.
  3. Archived sessions (a session dir containing an `ARCHIVED` marker file) are
     never auto-selected and are excluded from the active list.
  4. No shell=True, no f-string command building — argv is a list, always.
"""
import os
import sys
import json
import shlex
import subprocess
from datetime import datetime, timezone
from pathlib import Path

import pandas as pd
import streamlit as st

sys.path.insert(0, str(Path(__file__).resolve().parent))
from file_lock import file_lock, LockError  # noqa: E402  (shared advisory lock helper)

REPO = Path(__file__).resolve().parent.parent
SESSIONS_DIR = REPO / "doc" / "collab" / "sessions"
TASKS_PATH = REPO / "doc" / "collab" / "tasks.md"
ARCHIVE_MARKER = "ARCHIVED"
CLAUDE_BIN = os.environ.get("CLAUDE_BIN", "claude")  # `claude` may not be on PATH; set this
# Constrained toolset for invoked agents. NO Bash -> agent cannot invoke peers / loop.
# Verify exact flag names against your Claude Code version; an unknown flag fails
# safe (the run errors out, no agent acts) rather than dangerously.
ALLOWED_TOOLS = ["Read", "Edit", "Write", "Grep", "Glob"]
DISALLOWED_TOOLS = ["Bash"]
AGENT_TIMEOUT_S = 600

st.set_page_config(page_title="Deep Cuts Collab Hub", layout="wide")


# ── session discovery (tombstone-aware) ─────────────────────────────────────
def is_archived(d: Path) -> bool:
    return (d / ARCHIVE_MARKER).exists()


def list_sessions(include_archived: bool = False):
    if not SESSIONS_DIR.exists():
        return []
    ds = [d for d in SESSIONS_DIR.iterdir() if d.is_dir() and not d.name.startswith(".")]
    if not include_archived:
        ds = [d for d in ds if not is_archived(d)]
    ds.sort(key=lambda d: d.stat().st_mtime, reverse=True)  # most recent first
    return ds


# ── chat log I/O ────────────────────────────────────────────────────────────
def append_message(path: Path, sender: str, content: str, msg_type: str = "markdown", artifact: str = None):
    msg = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "sender": sender,
        "type": msg_type,
        "content": content,
    }
    if artifact:
        msg["path"] = artifact
    data = (json.dumps(msg) + "\n").encode("utf-8")
    # Single O_APPEND write is atomic for small lines -> safe for concurrent writers.
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
    try:
        os.write(fd, data)
    finally:
        os.close(fd)


def read_chat_log(path: Path):
    out = []
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line:
                try:
                    out.append(json.loads(line))
                except Exception:
                    pass
    return out


# ── sidebar: session selection + archive controls ──────────────────────────
active = list_sessions()
if not active:
    st.error("No active (non-archived) session under `doc/collab/sessions/`.")
    st.stop()

st.sidebar.title("Collab Hub")
choice = st.sidebar.selectbox("Active session", [d.name for d in active], index=0)
session_dir = next(d for d in active if d.name == choice)
chat_log_path = session_dir / "chat_log.jsonl"
session_md_path = session_dir / "session.md"

c1, c2 = st.sidebar.columns(2)
if c1.button("📦 Archive this"):
    (session_dir / ARCHIVE_MARKER).write_text(f"archived {datetime.now().isoformat()}\n")
    st.rerun()
with st.sidebar.expander("Archived sessions"):
    archived = [d for d in list_sessions(include_archived=True) if is_archived(d)]
    if not archived:
        st.caption("none")
    for d in archived:
        if st.button(f"Unarchive {d.name}", key=f"un_{d.name}"):
            (d / ARCHIVE_MARKER).unlink(missing_ok=True)
            st.rerun()


# ── chat rendering (live, rich formats) ─────────────────────────────────────
def render_message(msg):
    with st.chat_message(msg.get("sender", "?").lower()):
        st.caption(f"{msg.get('sender', '?')} · {msg.get('timestamp', '')}")
        mtype = msg.get("type", "markdown")
        rel = msg.get("path")
        if mtype == "image" and rel:
            st.image(str(session_dir / rel), caption=msg.get("content") or rel)
        elif mtype == "dataset" and rel:
            fp = session_dir / rel
            if msg.get("content"):
                st.markdown(msg["content"])
            try:
                df = pd.read_csv(fp) if fp.suffix.lower() == ".csv" else pd.read_json(fp)
                st.dataframe(df, use_container_width=True)
            except Exception as e:
                st.warning(f"Could not render dataset `{rel}`: {e}")
        else:  # markdown / text
            st.markdown(msg.get("content", ""))


@st.fragment(run_every="5s")
def tasks_pane():
    with st.expander("📋 doc/collab/tasks.md (live)", expanded=False):
        if TASKS_PATH.exists():
            st.markdown(TASKS_PATH.read_text(encoding="utf-8"))
        else:
            st.caption("No `doc/collab/tasks.md` yet.")


@st.fragment(run_every="2s")
def chat_pane():
    msgs = read_chat_log(chat_log_path)
    if not msgs:
        st.caption("No messages yet — say something below, or attach an artifact.")
    for m in msgs:
        render_message(m)


st.title("💬 Multi-Agent Collaboration Hub")
st.caption(f"Session: `{session_dir.name}` · live (2s)")
tasks_pane()
chat_pane()

if prompt := st.chat_input("Message…"):
    append_message(chat_log_path, "Roberto", prompt, "markdown")
    st.rerun()


# ── attach rich artifacts (markdown / dataset / image) ──────────────────────
with st.sidebar.expander("📎 Attach artifact"):
    up = st.file_uploader("CSV / JSON / PNG / JPG / MD", type=["csv", "json", "png", "jpg", "jpeg", "md"])
    note = st.text_input("Caption / note", key="artifact_note")
    if up is not None and st.button("Post artifact"):
        att = session_dir / "attachments"
        att.mkdir(exist_ok=True)
        dest = att / up.name
        dest.write_bytes(up.getvalue())
        rel = str(dest.relative_to(session_dir))
        ext = dest.suffix.lower()
        if ext == ".md":
            append_message(chat_log_path, "Roberto", dest.read_text(encoding="utf-8"), "markdown")
        elif ext in (".png", ".jpg", ".jpeg"):
            append_message(chat_log_path, "Roberto", note or up.name, "image", artifact=rel)
        else:  # csv / json
            append_message(chat_log_path, "Roberto", note or up.name, "dataset", artifact=rel)
        st.rerun()


# ── invoke agent: human-gated, one turn, constrained permissions ────────────
st.sidebar.subheader("Invoke agent (one turn)")
with st.sidebar.form("invoke"):
    default_prompt = (
        f"Read chat_log.jsonl in doc/collab/sessions/{session_dir.name}/ and append exactly "
        f"one reply (as a new JSON line) to the latest messages. Do not invoke any other agent."
    )
    p_text = st.text_area("Prompt", value=default_prompt, height=140)
    go = st.form_submit_button("Invoke Claude")
    if go:
        # argv list, no shell=True, no --dangerously-skip-permissions, Bash blocked.
        cmd = [CLAUDE_BIN, "-p", p_text, "--output-format", "json",
               "--allowedTools", *ALLOWED_TOOLS,
               "--disallowedTools", *DISALLOWED_TOOLS]
        st.code(" ".join(shlex.quote(c) for c in cmd), language="bash")
        try:
            with st.spinner("Claude is taking one turn…"):
                res = subprocess.run(cmd, cwd=str(REPO), capture_output=True,
                                     text=True, timeout=AGENT_TIMEOUT_S)
            if res.returncode != 0:
                st.error(f"claude exited {res.returncode}:\n{(res.stderr or '')[:800]}")
            else:
                reply = res.stdout
                try:
                    reply = json.loads(res.stdout).get("result", res.stdout)
                except Exception:
                    pass
                # If the agent already appended its own line, this just adds a mirror;
                # prefer the agent writing the file itself, but capture as a fallback.
                append_message(chat_log_path, "Claude", reply, "markdown")
                st.rerun()
        except FileNotFoundError:
            st.error(f"`{CLAUDE_BIN}` not found. Set the CLAUDE_BIN env var to your claude binary path.")
        except subprocess.TimeoutExpired:
            st.error(f"Claude turn exceeded {AGENT_TIMEOUT_S}s and was stopped.")

st.sidebar.caption("Gemini / Meta: paste their reply into the chat manually (they can't write here).")


# ── promote to session.md (range-based, lock-protected) ─────────────────────
st.sidebar.subheader("Promote to session.md")
if st.sidebar.button("Promote new messages"):
    msgs = read_chat_log(chat_log_path)
    hw = session_dir / ".promoted_count"
    start = int(hw.read_text()) if hw.exists() else 0
    new = msgs[start:]
    if not new:
        st.sidebar.info("Nothing new to promote.")
    else:
        try:
            with file_lock(session_md_path, owner="collab_hub"):
                with open(session_md_path, "a", encoding="utf-8") as f:
                    f.write(f"\n---\n\n## [Collab Hub Sync, {datetime.now().strftime('%H:%M')}]\n\n")
                    for m in new:
                        extra = f" (attachment: `{m['path']}`)" if m.get("path") else ""
                        f.write(f"**{m.get('sender','?')}**: {m.get('content','')}{extra}\n\n")
                hw.write_text(str(len(msgs)))
            st.sidebar.success(f"Promoted {len(new)} message(s) to session.md.")
        except LockError as e:
            st.sidebar.error(str(e))
