"""Maildir-backed message + task store for the collab coordination protocol.

Pure-stdlib implementation of the contract in doc/collab/coordination-protocol.md.
No MCP dependency lives here, so the store is unit-testable standalone; server.py
is the thin FastMCP wrapper.

Layout (all under `root`):
    tmp/                     write-staging (atomic-rename source)
    <actor>/new/             delivered, unread   (watched)
    <actor>/cur/             read/processed      (audit trail)
    tasks/open/              postable work
    tasks/claimed/<actor>/   atomically claimed
    tasks/done/              completed (+ result)

Atomicity comes from os.replace / os.rename within one filesystem: `send` is
write-then-rename (no partial reads), and `claim` is a rename race where exactly
one worker wins (the loser sees the source already gone).
"""

from __future__ import annotations

import json
import os
import time
import uuid
from pathlib import Path
from typing import Any, Optional

try:  # event-driven recv when available; poll fallback otherwise
    from watchfiles import watch as _watch
except Exception:  # pragma: no cover - optional dependency
    _watch = None


LEASE_TTL_DEFAULT = 120.0  # seconds; a claimed task auto-returns via sweep() after this


def _now_ns() -> int:
    return time.time_ns()


def _new_filename(msg_id: str) -> str:
    # zero-padded ns prefix => lexicographic sort == chronological order
    return f"{_now_ns():020d}-{msg_id}.json"


class MailStore:
    """One actor's view of a shared coordination root."""

    def __init__(self, root: str | os.PathLike, actor: str) -> None:
        self.root = Path(root)
        self.actor = actor
        self.tmp = self.root / "tmp"
        for p in (
            self.tmp,
            self._mbox("new"),
            self._mbox("cur"),
            self.root / "tasks" / "open",
            self.root / "tasks" / "claimed" / actor,
            self.root / "tasks" / "done",
        ):
            p.mkdir(parents=True, exist_ok=True)

    # -- helpers ----------------------------------------------------------
    def _mbox(self, sub: str, actor: Optional[str] = None) -> Path:
        return self.root / (actor or self.actor) / sub

    def _atomic_write(self, dest: Path, obj: dict) -> Path:
        dest.parent.mkdir(parents=True, exist_ok=True)
        staging = self.tmp / f".{uuid.uuid4().hex}.tmp"
        staging.write_text(json.dumps(obj, indent=2, sort_keys=True))
        os.replace(staging, dest)  # atomic within the same filesystem
        return dest

    @staticmethod
    def _read(p: Path) -> Optional[dict]:
        try:
            return json.loads(p.read_text())
        except (OSError, json.JSONDecodeError):
            return None

    # -- mailbox ----------------------------------------------------------
    def send(self, to: str, type: str, payload: Any, in_reply_to: Optional[str] = None) -> dict:
        env: dict = {
            "id": uuid.uuid4().hex,
            "from": self.actor,
            "to": to,
            "type": type,
            "payload": payload,
            "ts": time.time(),
        }
        if in_reply_to:
            env["in_reply_to"] = in_reply_to
        self._atomic_write(self._mbox("new", to) / _new_filename(env["id"]), env)
        return env

    def _pending_messages(self, match_type: Optional[str] = None) -> list[tuple[float, str, Path, dict]]:
        pending: list[tuple[float, str, Path, dict]] = []
        for p in self._mbox("new").glob("*.json"):
            env = self._read(p)
            if env is None or (match_type is not None and env.get("type") != match_type):
                continue
            pending.append((float(env.get("ts", 0)), p.name, p, env))
        return sorted(pending)

    def try_recv(self, match_type: Optional[str] = None) -> Optional[dict]:
        for _, _, p, env in self._pending_messages(match_type):
            try:
                os.replace(p, self._mbox("cur") / p.name)  # take it (new -> cur)
            except FileNotFoundError:
                continue  # already taken by a concurrent recv
            return env
        return None

    def recv(self, match_type: Optional[str] = None, timeout_s: Optional[float] = None) -> Optional[dict]:
        deadline = None if timeout_s is None else time.monotonic() + timeout_s
        env = self.try_recv(match_type)
        if env is not None:
            return env
        newdir = str(self._mbox("new"))
        while True:
            remaining = None if deadline is None else deadline - time.monotonic()
            if remaining is not None and remaining <= 0:
                return None
            self._wait_for_change(newdir, remaining)
            env = self.try_recv(match_type)
            if env is not None:
                return env

    @staticmethod
    def _wait_for_change(path: str, timeout_s: Optional[float]) -> None:
        # cap each wait at ~1s so a missed fs event is still caught by the re-check
        step = 1.0 if timeout_s is None else min(1.0, max(0.05, timeout_s))
        if _watch is None:
            time.sleep(min(0.2, step))
            return
        try:
            next(_watch(path, rust_timeout=int(step * 1000), yield_on_timeout=True), None)
        except Exception:
            time.sleep(min(0.2, step))

    # -- task queue (protected object) ------------------------------------
    def post(self, payload: Any, type: str = "task") -> dict:
        env: dict = {
            "id": uuid.uuid4().hex,
            "from": self.actor,
            "type": type,
            "payload": payload,
            "ts": time.time(),
            "status": "open",
        }
        self._atomic_write(self.root / "tasks" / "open" / _new_filename(env["id"]), env)
        return env

    def claim(self, lease_ttl: Optional[float] = LEASE_TTL_DEFAULT) -> Optional[dict]:
        claimed_dir = self.root / "tasks" / "claimed" / self.actor
        claimed_dir.mkdir(parents=True, exist_ok=True)
        for p in sorted((self.root / "tasks" / "open").glob("*.json")):
            dest = claimed_dir / p.name
            try:
                os.rename(p, dest)  # atomic: exactly one worker wins
            except (FileNotFoundError, OSError):
                continue  # lost the race
            now = time.time()
            env = self._read(dest) or {}
            env.update(status="claimed", owner=self.actor, claimed_at=now)
            if lease_ttl:
                env["lease_expires_at"] = now + lease_ttl
            self._atomic_write(dest, env)
            return env
        return None

    def _find_claimed(self, task_id: str) -> tuple[Optional[Path], Optional[dict]]:
        """Locate a task in this actor's claimed/ dir by envelope id, not filename.

        Identity lives in env["id"]; filenames vary across adapters, so never
        reconstruct a path from task_id.
        """
        claimed_dir = self.root / "tasks" / "claimed" / self.actor
        for p in claimed_dir.glob("*.json"):
            env = self._read(p)
            if env is not None and env.get("id") == task_id:
                return p, env
        return None, None

    def complete(self, task_id: str, result: Any) -> Optional[dict]:
        p, env = self._find_claimed(task_id)
        if env is None:
            return None
        env.update(status="done", result=result, completed_at=time.time())
        self._atomic_write(self.root / "tasks" / "done" / p.name, env)
        p.unlink(missing_ok=True)
        return env

    def heartbeat(self, task_id: str, lease_ttl: float = LEASE_TTL_DEFAULT) -> Optional[dict]:
        """Extend the lease on a task this actor holds, so sweep() won't reclaim it."""
        p, env = self._find_claimed(task_id)
        if env is None:
            return None
        env["lease_expires_at"] = time.time() + lease_ttl
        self._atomic_write(p, env)
        return env

    def abandon(self, task_id: str, reason: str) -> Optional[dict]:
        """Release a task this actor holds back to open/ with a diagnostic reason."""
        p, env = self._find_claimed(task_id)
        if env is None:
            return None
        env["status"] = "open"
        for k in ("owner", "claimed_at", "lease_expires_at"):
            env.pop(k, None)
        env["abandoned_by"] = self.actor
        env["abandoned_reason"] = reason
        env["abandoned_at"] = time.time()
        self._atomic_write(self.root / "tasks" / "open" / p.name, env)
        p.unlink(missing_ok=True)
        return env

    def sweep(self) -> list[dict]:
        """Coordinator op: return tasks whose lease has expired back to open/.

        Scans every actor's claimed/ dir (not just this one), so a coordinator can
        reclaim work abandoned by a crashed worker. Returns the reclaimed envelopes.
        """
        now = time.time()
        reclaimed: list[dict] = []
        claimed_root = self.root / "tasks" / "claimed"
        if not claimed_root.exists():
            return reclaimed
        for actor_dir in sorted(claimed_root.glob("*")):
            if not actor_dir.is_dir():
                continue
            for p in actor_dir.glob("*.json"):
                env = self._read(p)
                if env is None:
                    continue
                expires = env.get("lease_expires_at")
                if expires is None or expires > now:
                    continue  # no lease or still valid
                env["status"] = "open"
                env["swept_from"] = env.pop("owner", None)
                env.pop("claimed_at", None)
                env.pop("lease_expires_at", None)
                env["swept_at"] = now
                self._atomic_write(self.root / "tasks" / "open" / p.name, env)
                p.unlink(missing_ok=True)
                reclaimed.append(env)
        return reclaimed

    # -- tracing ----------------------------------------------------------
    def inbox(self, actor: Optional[str] = None) -> dict:
        who = actor or self.actor
        tasks = self.root / "tasks"

        def names(path: Path) -> list[str]:
            return sorted(p.name for p in path.glob("*.json")) if path.exists() else []

        return {
            "actor": who,
            "new": names(self.root / who / "new"),
            "cur_recent": names(self.root / who / "cur")[-10:],
            "tasks_open": len(names(tasks / "open")),
            "tasks_claimed": {
                d.name: len(names(d))
                for d in sorted((tasks / "claimed").glob("*"))
                if d.is_dir()
            },
            "tasks_done": len(names(tasks / "done")),
        }
