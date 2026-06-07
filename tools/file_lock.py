#!/usr/bin/env python3
"""
Advisory file lock for the multi-agent collaboration workflow.

"I temporarily own this file." Take the lock before editing a *shared* file
(`session.md`, `chat_log.jsonl`, `PROTOCOL.md`, shared docs) so Claude, Gemini,
and Roberto don't clobber each other's writes.

It is *advisory* (cooperative): it only works if every writer checks it. It is
not an OS mandatory lock. The lock is a `<path>.lock` sidecar holding
{owner, pid, ts}. Locks older than `expiry` seconds (default 120) are stale and
may be reclaimed — so a crashed agent never wedges a file permanently.

Native use (Python — Claude/Gemini scripts, the Collab Hub):
    from file_lock import file_lock
    with file_lock("doc/collab/sessions/.../session.md", owner="claude"):
        ...  # edit the file

CLI use (any agent / shell):
    python tools/file_lock.py acquire <path> --owner claude
    python tools/file_lock.py release <path> --owner claude
    python tools/file_lock.py status  <path>
Exit code 1 (and "LOCKED: ...") if another live owner holds it.
"""
import os
import sys
import json
import time
import argparse
import contextlib
from pathlib import Path

DEFAULT_EXPIRY = 120


class LockError(RuntimeError):
    pass


def _lock_path(target) -> Path:
    return Path(str(target) + ".lock")


def _read(lock: Path):
    try:
        return json.loads(lock.read_text())
    except Exception:
        return None


def status(target, expiry: int = DEFAULT_EXPIRY):
    """Return the live holder's metadata, or None if free/stale."""
    lock = _lock_path(target)
    if not lock.exists():
        return None
    meta = _read(lock)
    if not meta or time.time() - meta.get("ts", 0) > expiry:
        return None  # unreadable or stale -> treat as free
    return meta


def acquire(target, owner: str = "agent", expiry: int = DEFAULT_EXPIRY):
    lock = _lock_path(target)
    payload = json.dumps({"owner": owner, "pid": os.getpid(), "ts": time.time()})
    try:
        # O_EXCL: atomic create-if-absent — the actual mutual-exclusion primitive.
        fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
        os.write(fd, payload.encode())
        os.close(fd)
        return True
    except FileExistsError:
        held = status(target, expiry)
        if held and held.get("owner") != owner:
            raise LockError(
                f"{Path(target).name} is locked by {held.get('owner')} "
                f"(held {int(time.time() - held.get('ts', 0))}s)"
            )
        # stale, unreadable, or already ours -> reclaim and retry once
        lock.unlink(missing_ok=True)
        fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
        os.write(fd, payload.encode())
        os.close(fd)
        return True


def release(target, owner: str = None):
    lock = _lock_path(target)
    if lock.exists():
        meta = _read(lock)
        if owner and meta and meta.get("owner") != owner:
            raise LockError(
                f"refusing to release {Path(target).name}: owned by "
                f"{meta.get('owner')}, not {owner}"
            )
        lock.unlink(missing_ok=True)
    return True


@contextlib.contextmanager
def file_lock(target, owner: str = "agent", expiry: int = DEFAULT_EXPIRY):
    acquire(target, owner, expiry)
    try:
        yield
    finally:
        release(target, owner)


def _main(argv=None):
    ap = argparse.ArgumentParser(description="Advisory file lock for collaboration.")
    ap.add_argument("action", choices=["acquire", "release", "status"])
    ap.add_argument("path")
    ap.add_argument("--owner", default="agent")
    ap.add_argument("--expiry", type=int, default=DEFAULT_EXPIRY)
    a = ap.parse_args(argv)
    try:
        if a.action == "acquire":
            acquire(a.path, a.owner, a.expiry)
            print(f"acquired {a.path} as {a.owner}")
        elif a.action == "release":
            release(a.path, a.owner)
            print(f"released {a.path}")
        else:
            held = status(a.path, a.expiry)
            print(json.dumps(held) if held else "free")
    except LockError as e:
        print(f"LOCKED: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    _main()
