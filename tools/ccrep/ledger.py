"""Append-only event ledger for CCREP (the blackboard storage layer).

Pure-stdlib ``sqlite3`` (WAL), no MCP dependency — same convention as
``collab_mcp.store``: this module is unit-testable on its own; ``server.py`` is
the thin FastMCP wrapper.

The ``event_log`` table is the single source of truth. Materialized tables
(``proposals``, ``evaluation_reports``, ``critiques``, ``votes``,
``merge_records``) are DERIVED by the reducer folding the log — agents never
write consensus state directly (invariant 5). This module owns only:

  * appending events,
  * a content-addressed eval cache keyed
    ``(commit_sha, eval_suite_hash, dataset_hash, env_hash)`` (invariant 2),
  * (re)materializing the derived tables from a reducer-supplied snapshot.

The reduce logic itself lives in ``reducer.py`` so the storage layer stays a
dumb, deterministic append/fold substrate.

Path: env ``CCREP_DB`` or, by default, ``scratch/ccrep.db`` resolved against the
canonical (primary-worktree) repo root — the SAME file the MCP launcher
``tools/run_ccrep_mcp.py`` selects, so every linked worktree shares one ledger
regardless of CWD (KNOWN_ISSUES #1). macOS/APFS only — no WSL2/BTRFS fallback
machinery (the synthesis explicitly drops it).
"""

from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import subprocess
import time
import uuid
from pathlib import Path
from typing import Any, Iterable, Optional

# Ledger path relative to the canonical repo root. Kept in sync with
# tools/run_ccrep_mcp.py so direct invocation and the MCP launcher resolve to the
# same absolute file (KNOWN_ISSUES #1: the old "scratch/ccrep/ccrep.db" default
# diverged from the launcher's "scratch/ccrep.db", splitting the ledger).
DEFAULT_DB_RELATIVE = "scratch/ccrep.db"
# Back-compat alias; db_path() resolves it canonically.
DEFAULT_DB = DEFAULT_DB_RELATIVE

# Event kinds the reducer understands. Storage accepts any string, but these are
# the Phase-1 vocabulary.
EVENT_TASK_CLAIMED = "task_claimed"
EVENT_PROPOSAL_SUBMITTED = "proposal_submitted"
EVENT_EVALUATION_COMPLETED = "evaluation_completed"
EVENT_CRITIQUE_SUBMITTED = "critique_submitted"
EVENT_REVISION_SUBMITTED = "revision_submitted"
EVENT_MERGE_RECORDED = "merge_recorded"


def _canonical_repo_root() -> Path:
    """Primary-worktree root shared by linked git worktrees (mirrors
    ``tools/run_ccrep_mcp.py:canonical_repo_root``). Falls back to CWD when not in
    a git repo so unit tests and ad-hoc runs still work."""
    try:
        common = subprocess.check_output(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return Path.cwd()
    git_dir = Path(common)
    return git_dir.parent if git_dir.name == ".git" else Path.cwd()


def db_path() -> Path:
    """Canonical ledger path. ``CCREP_DB`` wins when set; otherwise the default is
    resolved against the shared repo root so all worktrees use one ledger."""
    env = os.environ.get("CCREP_DB")
    if env:
        return Path(env)
    return _canonical_repo_root() / DEFAULT_DB_RELATIVE


def content_hash(*parts: str) -> str:
    """sha256 over the eval-cache 4-tuple (commit, suite, dataset, env)."""
    h = hashlib.sha256()
    for p in parts:
        h.update(p.encode("utf-8"))
        h.update(b"\x00")
    return h.hexdigest()


_SCHEMA = """
CREATE TABLE IF NOT EXISTS event_log (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id     TEXT NOT NULL UNIQUE,
    task_id      TEXT NOT NULL,
    kind         TEXT NOT NULL,
    actor        TEXT,
    ts           REAL NOT NULL,
    payload      TEXT NOT NULL                 -- JSON
);
CREATE INDEX IF NOT EXISTS idx_event_task ON event_log(task_id, seq);

-- Content-addressed eval cache (invariant 2). Never re-run an eval whose inputs
-- are unchanged: key is the 4-tuple hash.
CREATE TABLE IF NOT EXISTS eval_cache (
    cache_key    TEXT PRIMARY KEY,            -- sha256(commit, suite, dataset, env)
    commit_sha   TEXT NOT NULL,
    suite_hash   TEXT NOT NULL,
    dataset_hash TEXT NOT NULL,
    env_hash     TEXT NOT NULL,
    report       TEXT NOT NULL,               -- JSON EvaluationReport
    created_at   REAL NOT NULL
);

-- Derived/materialized tables. Written ONLY by materialize(); never by agents.
CREATE TABLE IF NOT EXISTS proposals (
    proposal_id  TEXT PRIMARY KEY,
    task_id      TEXT NOT NULL,
    revision     INTEGER NOT NULL,
    author       TEXT NOT NULL,
    commit_sha   TEXT NOT NULL,
    status       TEXT NOT NULL,
    snapshot     TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS evaluation_reports (
    report_id    TEXT PRIMARY KEY,
    proposal_id  TEXT NOT NULL,
    commit_sha   TEXT NOT NULL,
    status       TEXT NOT NULL,
    snapshot     TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS critiques (
    critique_id  TEXT PRIMARY KEY,
    proposal_id  TEXT NOT NULL,
    reviewer     TEXT NOT NULL,
    stance       TEXT NOT NULL,
    snapshot     TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS votes (
    vote_id      TEXT PRIMARY KEY,
    task_id      TEXT NOT NULL,
    proposal_id  TEXT NOT NULL,
    agent_id     TEXT NOT NULL,
    commit_sha   TEXT NOT NULL,               -- the commit the vote was cast against
    vote         TEXT NOT NULL,
    snapshot     TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS merge_records (
    proposal_id  TEXT PRIMARY KEY,
    task_id      TEXT NOT NULL,
    commit_sha   TEXT NOT NULL,
    merged_by    TEXT,
    snapshot     TEXT NOT NULL
);
"""


class Ledger:
    """Append-only event store + derived-view materializer."""

    def __init__(self, path: str | os.PathLike | None = None) -> None:
        self.path = Path(path) if path is not None else db_path()
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._conn = sqlite3.connect(str(self.path))
        self._conn.row_factory = sqlite3.Row
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute("PRAGMA foreign_keys=ON")
        self._conn.executescript(_SCHEMA)
        self._conn.commit()

    def close(self) -> None:
        self._conn.close()

    def __enter__(self) -> "Ledger":
        return self

    def __exit__(self, *exc: Any) -> None:
        self.close()

    # -- append-only log --------------------------------------------------
    def append(
        self,
        task_id: str,
        kind: str,
        payload: dict,
        actor: Optional[str] = None,
        event_id: Optional[str] = None,
        ts: Optional[float] = None,
    ) -> dict:
        """Append one immutable event. Returns the stored envelope."""
        env = {
            "event_id": event_id or uuid.uuid4().hex,
            "task_id": task_id,
            "kind": kind,
            "actor": actor,
            "ts": ts if ts is not None else time.time(),
            "payload": payload,
        }
        self._conn.execute(
            "INSERT INTO event_log (event_id, task_id, kind, actor, ts, payload)"
            " VALUES (?, ?, ?, ?, ?, ?)",
            (
                env["event_id"],
                task_id,
                kind,
                actor,
                env["ts"],
                json.dumps(payload, sort_keys=True),
            ),
        )
        self._conn.commit()
        return env

    def events(self, task_id: Optional[str] = None) -> list[dict]:
        """Return events in append order (the whole log, or one task's slice)."""
        if task_id is None:
            cur = self._conn.execute(
                "SELECT * FROM event_log ORDER BY seq ASC"
            )
        else:
            cur = self._conn.execute(
                "SELECT * FROM event_log WHERE task_id = ? ORDER BY seq ASC",
                (task_id,),
            )
        out = []
        for row in cur.fetchall():
            out.append(
                {
                    "seq": row["seq"],
                    "event_id": row["event_id"],
                    "task_id": row["task_id"],
                    "kind": row["kind"],
                    "actor": row["actor"],
                    "ts": row["ts"],
                    "payload": json.loads(row["payload"]),
                }
            )
        return out

    def task_ids(self) -> list[str]:
        cur = self._conn.execute(
            "SELECT DISTINCT task_id FROM event_log ORDER BY task_id"
        )
        return [r["task_id"] for r in cur.fetchall()]

    # -- content-addressed eval cache (invariant 2) -----------------------
    def cache_get(self, cache_key: str) -> Optional[dict]:
        cur = self._conn.execute(
            "SELECT report FROM eval_cache WHERE cache_key = ?", (cache_key,)
        )
        row = cur.fetchone()
        return json.loads(row["report"]) if row else None

    def cache_put(
        self,
        cache_key: str,
        commit_sha: str,
        suite_hash: str,
        dataset_hash: str,
        env_hash: str,
        report: dict,
    ) -> None:
        self._conn.execute(
            "INSERT OR REPLACE INTO eval_cache"
            " (cache_key, commit_sha, suite_hash, dataset_hash, env_hash, report, created_at)"
            " VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                cache_key,
                commit_sha,
                suite_hash,
                dataset_hash,
                env_hash,
                json.dumps(report, sort_keys=True),
                time.time(),
            ),
        )
        self._conn.commit()

    # -- derived-view materialization -------------------------------------
    def materialize(self, snapshot: "MaterializedSnapshot") -> None:
        """Replace all derived tables from a reducer-produced snapshot.

        Derived state is never written by agents (invariant 5); only the reducer
        produces a snapshot and only this method persists it. Full-replace keeps
        the tables a pure function of the (immutable) event log.
        """
        c = self._conn
        c.execute("DELETE FROM proposals")
        c.execute("DELETE FROM evaluation_reports")
        c.execute("DELETE FROM critiques")
        c.execute("DELETE FROM votes")
        c.execute("DELETE FROM merge_records")

        for p in snapshot.proposals:
            c.execute(
                "INSERT INTO proposals"
                " (proposal_id, task_id, revision, author, commit_sha, status, snapshot)"
                " VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    p["proposal_id"],
                    p["task_id"],
                    p["revision"],
                    p["author"],
                    p["git"]["commit_sha"],
                    p["status"],
                    json.dumps(p, sort_keys=True),
                ),
            )
        for r in snapshot.evaluation_reports:
            c.execute(
                "INSERT INTO evaluation_reports"
                " (report_id, proposal_id, commit_sha, status, snapshot)"
                " VALUES (?, ?, ?, ?, ?)",
                (
                    r["report_id"],
                    r["proposal_id"],
                    r["commit_sha"],
                    r["status"],
                    json.dumps(r, sort_keys=True),
                ),
            )
        for cr in snapshot.critiques:
            c.execute(
                "INSERT INTO critiques"
                " (critique_id, proposal_id, reviewer, stance, snapshot)"
                " VALUES (?, ?, ?, ?, ?)",
                (
                    cr["critique_id"],
                    cr["proposal_id"],
                    cr["reviewer"],
                    cr["stance"],
                    json.dumps(cr, sort_keys=True),
                ),
            )
        for v in snapshot.votes:
            c.execute(
                "INSERT INTO votes"
                " (vote_id, task_id, proposal_id, agent_id, commit_sha, vote, snapshot)"
                " VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    v["vote_id"],
                    v["task_id"],
                    v["proposal_id"],
                    v["agent_id"],
                    v["commit_sha"],
                    v["vote"],
                    json.dumps(v, sort_keys=True),
                ),
            )
        for m in snapshot.merge_records:
            c.execute(
                "INSERT INTO merge_records"
                " (proposal_id, task_id, commit_sha, merged_by, snapshot)"
                " VALUES (?, ?, ?, ?, ?)",
                (
                    m["proposal_id"],
                    m["task_id"],
                    m["commit_sha"],
                    m.get("merged_by"),
                    json.dumps(m, sort_keys=True),
                ),
            )
        c.commit()


class MaterializedSnapshot:
    """A reducer's fold output, ready for :meth:`Ledger.materialize`."""

    def __init__(
        self,
        proposals: Iterable[dict] = (),
        evaluation_reports: Iterable[dict] = (),
        critiques: Iterable[dict] = (),
        votes: Iterable[dict] = (),
        merge_records: Iterable[dict] = (),
    ) -> None:
        self.proposals = list(proposals)
        self.evaluation_reports = list(evaluation_reports)
        self.critiques = list(critiques)
        self.votes = list(votes)
        self.merge_records = list(merge_records)
