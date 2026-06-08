"""Ledger storage tests: append-only log, eval cache, materialization (no MCP)."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

from . import ledger as ledger_mod
from .ledger import Ledger, MaterializedSnapshot, content_hash


def _tmp_db() -> Path:
    d = tempfile.mkdtemp(prefix="ccrep_ledger_")
    return Path(d) / "ccrep.db"


def test_db_path_env_override_wins(monkeypatch):
    monkeypatch.setenv("CCREP_DB", "/tmp/explicit/ccrep.db")
    assert ledger_mod.db_path() == Path("/tmp/explicit/ccrep.db")


def test_db_path_default_is_canonical_shared_ledger(monkeypatch):
    # KNOWN_ISSUES #1: with no CCREP_DB, the default must resolve to the SAME
    # absolute file the launcher uses (canonical repo root + scratch/ccrep.db),
    # not a CWD-relative "scratch/ccrep/ccrep.db" that splits the ledger.
    monkeypatch.delenv("CCREP_DB", raising=False)
    root = Path("/Users/example/repo")
    monkeypatch.setattr(ledger_mod, "_canonical_repo_root", lambda: root)
    assert ledger_mod.db_path() == root / "scratch" / "ccrep.db"
    # The launcher relative path and the ledger default agree on one filename.
    assert ledger_mod.DEFAULT_DB_RELATIVE == "scratch/ccrep.db"


def test_db_path_default_is_cwd_independent(monkeypatch, tmp_path):
    # Resolution must not depend on CWD (two worktrees -> one ledger).
    monkeypatch.delenv("CCREP_DB", raising=False)
    root = Path("/Users/example/repo")
    monkeypatch.setattr(ledger_mod, "_canonical_repo_root", lambda: root)
    monkeypatch.chdir(tmp_path)
    assert ledger_mod.db_path() == root / "scratch" / "ccrep.db"


def test_append_and_read_in_order():
    with Ledger(_tmp_db()) as led:
        led.append("t1", "k", {"n": 1})
        led.append("t1", "k", {"n": 2})
        led.append("t2", "k", {"n": 3})
        t1 = led.events("t1")
        assert [e["payload"]["n"] for e in t1] == [1, 2], "per-task order is append order"
        assert len(led.events()) == 3, "full log has all events"
        assert set(led.task_ids()) == {"t1", "t2"}


def test_event_ids_are_unique():
    with Ledger(_tmp_db()) as led:
        a = led.append("t", "k", {})
        b = led.append("t", "k", {})
        assert a["event_id"] != b["event_id"]


def test_eval_cache_roundtrip_is_content_addressed():
    with Ledger(_tmp_db()) as led:
        key = content_hash("a" * 40, "suite", "data", "env")
        assert led.cache_get(key) is None
        report = {"report_id": "r1", "status": "passed"}
        led.cache_put(key, "a" * 40, "suite", "data", "env", report)
        assert led.cache_get(key) == report
        # a different input tuple => different key => miss
        other = content_hash("b" * 40, "suite", "data", "env")
        assert led.cache_get(other) is None


def test_materialize_replaces_derived_tables():
    with Ledger(_tmp_db()) as led:
        snap = MaterializedSnapshot(
            proposals=[
                {
                    "proposal_id": "p1",
                    "task_id": "t",
                    "revision": 0,
                    "author": "claude",
                    "git": {"repo": "r", "commit_sha": "a" * 40},
                    "status": "submitted",
                }
            ],
            merge_records=[
                {"proposal_id": "p1", "task_id": "t", "commit_sha": "a" * 40, "merged_by": "x"}
            ],
        )
        led.materialize(snap)
        cur = led._conn.execute("SELECT proposal_id, status FROM proposals")
        rows = cur.fetchall()
        assert len(rows) == 1 and rows[0]["proposal_id"] == "p1"
        # re-materialize with an empty snapshot clears the tables (pure function)
        led.materialize(MaterializedSnapshot())
        assert led._conn.execute("SELECT COUNT(*) c FROM proposals").fetchone()["c"] == 0
        assert led._conn.execute("SELECT COUNT(*) c FROM merge_records").fetchone()["c"] == 0
