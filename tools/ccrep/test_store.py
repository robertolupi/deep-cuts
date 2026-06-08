"""End-to-end CcrepStore tests against a real throwaway git repo (no MCP runtime).

These exercise the worktree executor, content-addressed eval caching, critique
evidence-link resolution, and the human gate — the parts the pure-reducer tests
cannot reach. Run from tools/:

    .venv/bin/python -m pytest ccrep/test_store.py -q
"""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path

import pytest

from .store import CcrepStore


def _git(repo: Path, *args: str) -> str:
    cp = subprocess.run(
        ["git", *args], cwd=str(repo), capture_output=True, text=True, check=True
    )
    return cp.stdout.strip()


def _init_repo() -> tuple[Path, str]:
    """A tiny repo with one tracked file on a branch, returns (repo, branch)."""
    repo = Path(tempfile.mkdtemp(prefix="ccrep_repo_"))
    _git(repo, "init", "-q")
    _git(repo, "config", "user.email", "t@t.t")
    _git(repo, "config", "user.name", "t")
    _git(repo, "checkout", "-q", "-b", "work")
    (repo / "hello.txt").write_text("line1\nline2\nline3\n")
    _git(repo, "add", "hello.txt")
    _git(repo, "commit", "-q", "-m", "init")
    return repo, "work"


def _store(repo: Path) -> CcrepStore:
    db = repo / ".ccrep-db" / "ledger.db"
    return CcrepStore(repo_root=repo, db_path=db)


def test_submit_proposal_resolves_branch_to_commit():
    repo, branch = _init_repo()
    head = _git(repo, "rev-parse", branch)
    with _store(repo) as s:
        s.claim_task("t", "claude")
        res = s.submit_proposal(
            task_id="t",
            author="claude",
            branch=branch,
            artifact_profile="code_review",
            description="d",
            change_summary=["s"],
        )
        assert res["proposal"]["git"]["commit_sha"] == head, "branch resolved to commit"
        assert res["consensus"]["state"] in ("evaluating", "collecting_proposals", "reviewing")


def test_end_to_end_code_review_gate():
    # eval suite that always passes (echo), then an independent approval => mergeable
    repo, branch = _init_repo()
    with _store(repo) as s:
        s.claim_task("t", "claude")
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
        )["proposal"]
        ev = s.run_evaluation(
            prop["proposal_id"],
            suite_override=[{"name": "noop", "argv": ["true"]}],
        )
        assert ev["report"]["status"] == "passed"
        crit = {
            "proposal_id": prop["proposal_id"],
            "reviewer": "codex",
            "stance": "approve",
            "summary": "lgtm",
            "findings": [],
        }
        out = s.submit_critique(crit)
        assert out["consensus"]["decision"]["mergeable"] is True
        merged = s.merge_proposal(prop["proposal_id"], merged_by="roberto")
        assert merged["merged"] is True
        assert merged["consensus"]["state"] == "merged"


def test_eval_cache_hit_on_unchanged_inputs():
    repo, branch = _init_repo()
    with _store(repo) as s:
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
        )["proposal"]
        override = [{"name": "noop", "argv": ["true"]}]
        first = s.run_evaluation(prop["proposal_id"], suite_override=override)
        second = s.run_evaluation(prop["proposal_id"], suite_override=override)
        assert first["report"]["_cached"] is False
        assert second["report"]["_cached"] is True, "second eval served from cache"
        assert first["report"]["report_id"] == second["report"]["report_id"]


def test_failing_command_fails_the_gate():
    repo, branch = _init_repo()
    with _store(repo) as s:
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
        )["proposal"]
        ev = s.run_evaluation(
            prop["proposal_id"], suite_override=[{"name": "boom", "argv": ["false"]}]
        )
        assert ev["report"]["status"] == "failed"
        assert ev["consensus"]["decision"]["mergeable"] is False


def test_critique_with_dead_evidence_link_is_rejected():
    repo, branch = _init_repo()
    with _store(repo) as s:
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
        )["proposal"]
        crit = {
            "proposal_id": prop["proposal_id"],
            "reviewer": "codex",
            "stance": "request_changes",
            "summary": "bug",
            "findings": [
                {
                    "severity": "blocking",
                    "category": "correctness",
                    "claim": "x",
                    "evidence": [{"kind": "file_line", "uri": "hello.txt:999"}],
                }
            ],
        }
        with pytest.raises(Exception) as exc:
            s.submit_critique(crit)
        assert "does not resolve" in str(exc.value)


def test_critique_with_live_evidence_link_is_accepted():
    repo, branch = _init_repo()
    with _store(repo) as s:
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
        )["proposal"]
        crit = {
            "proposal_id": prop["proposal_id"],
            "reviewer": "codex",
            "stance": "request_changes",
            "summary": "bug",
            "findings": [
                {
                    "severity": "blocking",
                    "category": "correctness",
                    "claim": "x",
                    "evidence": [{"kind": "file_line", "uri": "hello.txt:2"}],
                }
            ],
        }
        out = s.submit_critique(crit)  # line 2 exists in hello.txt
        assert out["consensus"]["open_blocking_findings"], "blocking finding recorded"


def test_author_cannot_self_approve_via_store():
    repo, branch = _init_repo()
    with _store(repo) as s:
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
        )["proposal"]
        s.run_evaluation(prop["proposal_id"], suite_override=[{"name": "noop", "argv": ["true"]}])
        self_approve = {
            "proposal_id": prop["proposal_id"],
            "reviewer": "claude",  # == author
            "stance": "approve",
            "summary": "lgtm",
            "findings": [],
        }
        with pytest.raises(Exception) as exc:
            s.submit_critique(self_approve)
        assert "self-approval" in str(exc.value) or "invariant 4" in str(exc.value)


def test_human_gate_blocks_merge_without_confirmation():
    repo, branch = _init_repo()
    with _store(repo) as s:
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
            human_gate=["destructive_migration"],
        )["proposal"]
        s.run_evaluation(prop["proposal_id"], suite_override=[{"name": "noop", "argv": ["true"]}])
        s.submit_critique({
            "proposal_id": prop["proposal_id"], "reviewer": "codex",
            "stance": "approve", "summary": "ok", "findings": [],
        })
        blocked = s.merge_proposal(prop["proposal_id"], merged_by="codex")
        assert blocked["merged"] is False and blocked["requires_human"] is True
        confirmed = s.merge_proposal(
            prop["proposal_id"], merged_by="roberto", human_confirmed=True
        )
        assert confirmed["merged"] is True


def test_revision_invalidates_prior_approval_e2e():
    repo, branch = _init_repo()
    with _store(repo) as s:
        prop = s.submit_proposal(
            task_id="t", author="claude", branch=branch,
            artifact_profile="code_review", description="d", change_summary=["s"],
        )["proposal"]
        s.run_evaluation(prop["proposal_id"], suite_override=[{"name": "noop", "argv": ["true"]}])
        s.submit_critique({
            "proposal_id": prop["proposal_id"], "reviewer": "codex",
            "stance": "approve", "summary": "ok", "findings": [],
        })
        # new commit on the branch
        (repo / "hello.txt").write_text("line1\nline2\nline3\nline4\n")
        _git(repo, "add", "hello.txt")
        _git(repo, "commit", "-q", "-m", "rev")
        rev = s.submit_revision(
            previous_proposal_id=prop["proposal_id"], author="claude", branch=branch,
            artifact_profile="code_review", description="d2", change_summary=["s2"],
        )
        # head is the revision; old approval does not carry => not mergeable
        assert rev["consensus"]["decision"]["mergeable"] is False
        assert "independent approval" in rev["consensus"]["decision"]["reason"]
