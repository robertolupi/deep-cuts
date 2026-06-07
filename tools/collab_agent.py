#!/usr/bin/env python3
"""
Single chokepoint for running collaboration agents headlessly — with a KILL SWITCH.

Every headless agent turn (Collab Hub button, fish helpers, or by hand) should go through
here, so that:
  * Constraints live in ONE place: claude runs with a file read/write allowlist and Bash
    DISALLOWED (so it cannot shell out to invoke a peer -> no Claude<->Gemini loop); gemini
    (`agy`) runs `--sandbox`. Never --dangerously-skip-permissions.
  * Every agent runs in its OWN process group with a pidfile under .collab_agents/, so
    `kill` can terminate ALL running agents AND their children instantly. That's the runaway /
    token kill switch.
  * Every run has a wall-clock timeout (hard upper bound on a single turn).

Usage:
  python tools/collab_agent.py run claude  [--session NAME] ["prompt"]
  python tools/collab_agent.py run gemini  [--session NAME] ["prompt"]
  python tools/collab_agent.py kill        # STOP ALL running agents (kill switch)
  python tools/collab_agent.py status

With no prompt, a bootstrap "catch up and reply" prompt for the active session is used.
"""
import os
import sys
import json
import time
import signal
import argparse
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SESSIONS_DIR = REPO / "doc" / "collab" / "sessions"
RUN_DIR = REPO / ".collab_agents"            # pidfiles (gitignored)
TIMEOUT_S = int(os.environ.get("COLLAB_AGENT_TIMEOUT", "900"))
CLAUDE_BIN = os.environ.get("CLAUDE_BIN", os.path.expanduser("~/.local/bin/claude"))
AGY_BIN = os.environ.get("AGY_BIN", os.path.expanduser("~/.local/bin/agy"))
# claude: file tools only, Bash DISALLOWED -> cannot spawn peers/loop. (Verified-safe config.)
CLAUDE_ALLOWED = ["Read", "Edit", "Write", "Grep", "Glob"]
CLAUDE_DISALLOWED = ["Bash"]


def active_session():
    if not SESSIONS_DIR.exists():
        return None
    ds = [d for d in SESSIONS_DIR.iterdir()
          if d.is_dir() and not d.name.startswith(".") and not (d / "ARCHIVED").exists()]
    ds.sort(key=lambda d: d.stat().st_mtime)
    return ds[-1].name if ds else None


def bootstrap_prompt(session, sender):
    sess = f"doc/collab/sessions/{session}"
    return (
        f"Read doc/collab/PROTOCOL.md and {sess}/session.md first. "
        f"Messages in {sess}/chat_log.jsonl are JSON lines: "
        "{timestamp, sender, type, content}. "
        f'Append exactly one new line to {sess}/chat_log.jsonl as sender:"{sender}" replying to '
        "the latest message — use an append-only write (no lock needed for the log). "
        "Do not edit other shared files. Do not invoke other agents."
    )


def build_cmd(agent, prompt):
    if agent == "claude":
        return [CLAUDE_BIN, "-p", prompt, "--output-format", "json",
                "--allowedTools", *CLAUDE_ALLOWED, "--disallowedTools", *CLAUDE_DISALLOWED]
    if agent == "gemini":
        return [AGY_BIN, "--print", "--sandbox", prompt]
    raise SystemExit(f"unknown agent: {agent}")


def run(agent, session, prompt):
    session = session or active_session()
    if not session:
        raise SystemExit("no active (non-archived) session under doc/collab/sessions/")
    sender = "Claude" if agent == "claude" else "Gemini"
    prompt = prompt or bootstrap_prompt(session, sender)
    RUN_DIR.mkdir(exist_ok=True)
    cmd = build_cmd(agent, prompt)
    # start_new_session=True -> own process group; killpg takes down the whole tree.
    proc = subprocess.Popen(cmd, cwd=str(REPO), start_new_session=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    pidfile = RUN_DIR / f"{agent}-{proc.pid}.json"
    pidfile.write_text(json.dumps({
        "agent": agent, "pid": proc.pid, "pgid": os.getpgid(proc.pid),
        "started": time.time(), "session": session,
    }))
    try:
        out, err = proc.communicate(timeout=TIMEOUT_S)
        sys.stdout.write(out or "")
        if proc.returncode != 0:
            sys.stderr.write((err or "")[:1000])
        return proc.returncode
    except subprocess.TimeoutExpired:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except ProcessLookupError:
            pass
        sys.stderr.write(f"agent {agent} exceeded {TIMEOUT_S}s timeout — killed\n")
        return 124
    finally:
        pidfile.unlink(missing_ok=True)


def kill_all():
    killed = 0
    if RUN_DIR.exists():
        for pf in RUN_DIR.glob("*.json"):
            try:
                meta = json.loads(pf.read_text())
                pgid = meta.get("pgid")
                os.killpg(pgid, signal.SIGTERM)
                time.sleep(0.4)
                try:
                    os.killpg(pgid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                killed += 1
                print(f"killed {meta.get('agent')} pid {meta.get('pid')} (pgid {pgid})")
            except ProcessLookupError:
                pass
            except Exception as e:
                print(f"(could not kill from {pf.name}: {e})", file=sys.stderr)
            pf.unlink(missing_ok=True)
    print(f"KILL SWITCH: stopped {killed} tracked agent(s).")


def status():
    pfs = list(RUN_DIR.glob("*.json")) if RUN_DIR.exists() else []
    if not pfs:
        print("no running agents")
        return
    for pf in pfs:
        m = json.loads(pf.read_text())
        print(f"{m['agent']} pid={m['pid']} pgid={m['pgid']} session={m['session']} "
              f"running={int(time.time()-m['started'])}s")


def main():
    ap = argparse.ArgumentParser(description="Run/kill collaboration agents (kill switch).")
    sub = ap.add_subparsers(dest="action", required=True)
    r = sub.add_parser("run")
    r.add_argument("agent", choices=["claude", "gemini"])
    r.add_argument("prompt", nargs="?")
    r.add_argument("--session")
    sub.add_parser("kill")
    sub.add_parser("status")
    a = ap.parse_args()
    if a.action == "run":
        sys.exit(run(a.agent, a.session, a.prompt))
    elif a.action == "kill":
        kill_all()
    else:
        status()


if __name__ == "__main__":
    main()
