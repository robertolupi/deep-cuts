"""Worktree executor + eval-suite runner + critique evidence checks.

Pure-stdlib (subprocess/sqlite via the ledger), no MCP dependency. Three jobs:

  1. ``run_evaluation`` — check the proposal's pinned commit out into a detached
     git worktree, run the profile's eval suite, fold exit codes + output into a
     schema-valid ``EvaluationReport``, and content-address-cache it (invariant
     2). Robust cleanup: the worktree is always removed, even on failure.

  2. ``run_design_doc_checks`` — for the design_doc profile: lint_collab, a
     link check, skill-index consistency, provenance WARNINGS (never FAIL), and
     the one-directional frontmatter-status check (invariant 7).

  3. ``resolve_evidence_links`` — pre-review validity check on a Critique: each
     ``file_line`` evidence URI must resolve at the proposed ``commit_sha``. A
     dead link makes the critique malformed (rejected pre-review), which is
     mechanism, not judgment.

"Which suite runs is config" — the commands come from ``profiles.resolve_suite``
or a per-task override; the executor just runs them.
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import subprocess
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from . import schemas
from .ledger import Ledger, content_hash
from .profiles import (
    GATE_FRONTMATTER_STATUS,
    GATE_PROVENANCE_WARN,
    EvalCommand,
    get_profile,
    resolve_suite,
)

WORKTREE_ROOT = ".ccrep/worktrees"
# A two-decimal numeric claim like 0.92 / 99.27% — the provenance regex target.
_TWO_DECIMAL = re.compile(r"\b\d+\.\d{2,}%?\b")
# `path:line` evidence reference, e.g. src-tauri/src/lib.rs:42
_FILE_LINE = re.compile(r"^(?P<path>[^\s:]+):(?P<line>\d+)$")


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def _suite_hash(suite: list[EvalCommand]) -> str:
    payload = json.dumps(
        [[c.name, c.argv, c.required] for c in suite], sort_keys=True
    )
    return hashlib.sha256(payload.encode()).hexdigest()


def _env_hash(env_descriptor: str) -> str:
    return hashlib.sha256(env_descriptor.encode()).hexdigest()


class WorktreeExecutor:
    """Runs eval suites in disposable detached git worktrees."""

    def __init__(
        self,
        repo_root: str | Path,
        ledger: Ledger,
        env_descriptor: str = "macos-apfs-phase1",
    ) -> None:
        self.repo_root = Path(repo_root).resolve()
        self.ledger = ledger
        self.env_descriptor = env_descriptor

    # -- git helpers ------------------------------------------------------
    def _git(self, *args: str, cwd: Optional[Path] = None) -> subprocess.CompletedProcess:
        return subprocess.run(
            ["git", *args],
            cwd=str(cwd or self.repo_root),
            capture_output=True,
            text=True,
        )

    def resolve_commit(self, ref: str) -> str:
        """Resolve a branch/ref to its immutable 40+ char commit_sha (invariant 1)."""
        cp = self._git("rev-parse", ref)
        if cp.returncode != 0:
            raise ValueError(f"cannot resolve ref {ref!r}: {cp.stderr.strip()}")
        return cp.stdout.strip()

    def _add_worktree(self, proposal_id: str, commit_sha: str) -> Path:
        wt = self.repo_root / WORKTREE_ROOT / proposal_id
        if wt.exists():
            self._remove_worktree(wt)
        wt.parent.mkdir(parents=True, exist_ok=True)
        cp = self._git("worktree", "add", "--detach", str(wt), commit_sha)
        if cp.returncode != 0:
            raise RuntimeError(
                f"git worktree add failed: {cp.stderr.strip() or cp.stdout.strip()}"
            )
        return wt

    def _remove_worktree(self, wt: Path) -> None:
        # Robust cleanup: force-remove, then prune; never raise on cleanup.
        self._git("worktree", "remove", "--force", str(wt))
        if wt.exists():
            shutil.rmtree(wt, ignore_errors=True)
        self._git("worktree", "prune")

    # -- evaluation -------------------------------------------------------
    def run_evaluation(
        self,
        proposal: dict,
        suite_override: list[dict] | None = None,
        dataset_hash: str = "none",
        suite_id: Optional[str] = None,
        timeout_s: int = 1800,
        use_cache: bool = True,
    ) -> dict:
        """Evaluate a proposal in a worktree → EvaluationReport (cached by 4-tuple).

        Returns a schema-valid EvaluationReport dict. Carries a non-schema
        ``_components`` map (hard_check name → gate component) so the reducer can
        enforce invariant 6; ``_cached`` indicates a cache hit.
        """
        profile_name = proposal["artifact_profile"]
        profile = get_profile(profile_name)
        commit_sha = proposal["git"]["commit_sha"]
        suite = resolve_suite(profile_name, suite_override)
        suite_h = _suite_hash(suite)
        env_h = _env_hash(self.env_descriptor)
        suite_id = suite_id or f"{profile_name}-default"

        cache_key = content_hash(commit_sha, suite_h, dataset_hash, env_h)
        if use_cache:
            cached = self.ledger.cache_get(cache_key)
            if cached is not None:
                cached = dict(cached)
                cached["_cached"] = True
                return cached

        started = _now_iso()
        hard_checks: list[dict] = []
        components: dict[str, str] = {}
        status = "passed"

        # Map each command to the gate component it represents, for invariant 6.
        component_for = self._component_map(profile_name)

        wt: Optional[Path] = None
        try:
            wt = self._add_worktree(proposal["proposal_id"], commit_sha)
            for cmd in suite:
                check, comp = self._run_command(cmd, wt, timeout_s, component_for)
                hard_checks.append(check)
                components[check["name"]] = comp
                if cmd.required and not check["passed"]:
                    status = "failed"

            # design_doc gets the doc-specific checks layered on top.
            if profile_name == "design_doc":
                doc_checks, doc_components = self.run_design_doc_checks(proposal, wt)
                for dc in doc_checks:
                    hard_checks.append(dc)
                    components[dc["name"]] = doc_components[dc["name"]]
                    # provenance is WARN-only: never flips status to failed.
                    if (
                        not dc["passed"]
                        and doc_components[dc["name"]] != GATE_PROVENANCE_WARN
                    ):
                        status = "failed"
        except Exception as exc:  # pragma: no cover - defensive
            status = "error"
            hard_checks.append(
                {"name": "executor", "passed": False, "details": str(exc)}
            )
        finally:
            if wt is not None:
                self._remove_worktree(wt)

        completed = _now_iso()
        report = {
            "report_id": uuid.uuid4().hex,
            "proposal_id": proposal["proposal_id"],
            "commit_sha": commit_sha,
            "suite_id": suite_id,
            "suite_hash": suite_h,
            "environment_hash": env_h,
            "dataset_hash": dataset_hash,
            "started_at": started,
            "completed_at": completed,
            "status": status,
            "hard_checks": hard_checks,
            "metrics": {},
        }
        # Validate against the schema BEFORE attaching internal annotations.
        schemas.validate("EvaluationReport", report)
        report["_components"] = components
        report["_cached"] = False

        if use_cache and status in ("passed", "failed"):
            cacheable = {k: v for k, v in report.items() if not k.startswith("_")}
            cacheable["_components"] = components
            self.ledger.cache_put(
                cache_key, commit_sha, suite_h, dataset_hash, env_h, cacheable
            )
        return report

    def _component_map(self, profile_name: str) -> dict[str, str]:
        """Heuristic command-name → gate-component mapping for invariant 6 tagging."""
        from .profiles import GATE_BUILD_TEST, GATE_GOLDEN_METRIC, GATE_LINT_FMT

        return {
            "cargo_test": GATE_BUILD_TEST,
            "cargo_fmt_check": GATE_LINT_FMT,
            "cargo_clippy": GATE_LINT_FMT,
            "golden_metric": GATE_GOLDEN_METRIC,
        }

    def _run_command(
        self,
        cmd: EvalCommand,
        cwd: Path,
        timeout_s: int,
        component_for: dict[str, str],
    ) -> tuple[dict, str]:
        comp = component_for.get(cmd.name, "build_test")
        try:
            cp = subprocess.run(
                cmd.argv,
                cwd=str(cwd),
                capture_output=True,
                text=True,
                timeout=timeout_s,
            )
            passed = cp.returncode == 0
            tail = (cp.stdout + cp.stderr)[-2000:]
            check = {
                "name": cmd.name,
                "passed": passed,
                "details": f"exit={cp.returncode}\n{tail}".strip(),
            }
        except subprocess.TimeoutExpired:
            check = {
                "name": cmd.name,
                "passed": False,
                "details": f"timeout after {timeout_s}s",
            }
        except FileNotFoundError as exc:
            check = {
                "name": cmd.name,
                "passed": False,
                "details": f"command not found: {exc}",
            }
        return check, comp

    # -- design-doc checks ------------------------------------------------
    def run_design_doc_checks(
        self, proposal: dict, worktree: Path
    ) -> tuple[list[dict], dict[str, str]]:
        """Doc-linter eval suite for the design_doc profile.

        Returns (hard_checks, components). Provenance is a WARNING-only check
        (synthesis: detect, don't fail). The frontmatter check (invariant 7) IS
        a hard check — it fails only on accepted-without-APPROVED, never on the
        human's merge gesture.
        """
        checks: list[dict] = []
        components: dict[str, str] = {}

        # Provenance warning: flag unreferenced two-decimal numeric claims.
        doc_paths = proposal.get("changed_docs") or []
        flagged: list[str] = []
        for rel in doc_paths:
            fpath = worktree / rel
            if not fpath.exists():
                continue
            text = fpath.read_text(errors="ignore")
            for ln_no, line in enumerate(text.splitlines(), start=1):
                # A claim is "referenced" if the line also cites a source token.
                referenced = any(
                    tok in line.lower()
                    for tok in ("http", "eval", "report", "source", "ref", "[")
                )
                for m in _TWO_DECIMAL.finditer(line):
                    if not referenced:
                        flagged.append(f"{rel}:{ln_no}: {m.group(0)}")
        prov = {
            "name": "provenance_warnings",
            "passed": True,  # WARN-only: always passes, just lists findings
            "details": (
                "no unreferenced numeric claims"
                if not flagged
                else "WARN unreferenced numeric claims (review, do not auto-reject):\n"
                + "\n".join(flagged)
            ),
        }
        checks.append(prov)
        components[prov["name"]] = GATE_PROVENANCE_WARN

        # Frontmatter-status invariant (invariant 7), one-directional.
        fm_status = proposal.get("frontmatter_status")
        # The reducer owns whether APPROVED was reached; here we only fail the
        # *drift* shape: status: accepted asserted in the doc with no approval
        # signal recorded on the proposal.
        approved_signal = proposal.get("reached_approved", False)
        fm_passed = not (fm_status == "accepted" and not approved_signal)
        fm = {
            "name": "frontmatter_status",
            "passed": fm_passed,
            "details": (
                "frontmatter status consistent"
                if fm_passed
                else "invariant 7: status: accepted without a reached APPROVED state"
            ),
        }
        checks.append(fm)
        components[fm["name"]] = GATE_FRONTMATTER_STATUS

        return checks, components

    # -- critique evidence-link resolution --------------------------------
    def resolve_evidence_links(self, critique: dict, commit_sha: str) -> list[str]:
        """Verify each file_line evidence URI resolves at ``commit_sha``.

        Returns a list of unresolved/dead link descriptions; empty means valid.
        A non-empty result makes the critique malformed (reject pre-review) — this
        is mechanism, not judgment (synthesis §"Implementation Split").
        """
        problems: list[str] = []
        for finding in critique.get("findings", []):
            for ev in finding.get("evidence", []):
                if ev.get("kind") != "file_line":
                    continue
                uri = ev.get("uri", "")
                m = _FILE_LINE.match(uri)
                if not m:
                    problems.append(
                        f"{finding['finding_id']}: malformed file_line uri {uri!r} "
                        "(expected path:line)"
                    )
                    continue
                path, line = m.group("path"), int(m.group("line"))
                if not self._path_line_exists(commit_sha, path, line):
                    problems.append(
                        f"{finding['finding_id']}: {uri} does not resolve at "
                        f"{commit_sha[:12]}"
                    )
        return problems

    def _path_line_exists(self, commit_sha: str, path: str, line: int) -> bool:
        """True if blob ``path`` exists at ``commit_sha`` and has >= ``line`` lines."""
        if line < 1:
            return False
        cp = self._git("cat-file", "-p", f"{commit_sha}:{path}")
        if cp.returncode != 0:
            return False
        n_lines = cp.stdout.count("\n") + (0 if cp.stdout.endswith("\n") else 1)
        return line <= max(n_lines, 1)
