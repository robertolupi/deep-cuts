"""Read-only inspector for the CCREP evidence ledger.

A STABLE command surface so it can be allow-listed once
(``Bash(tools/ccrep_ledger.sh:*)``) instead of approving ad-hoc ``sqlite3`` /
``python -c`` one-liners every session. It NEVER writes ledger state: every
subcommand folds the append-only event log (``reduce_task``) and prints the
derived view.

Reach it through the wrapper:

    tools/ccrep_ledger.sh db                  # resolved canonical ledger path
    tools/ccrep_ledger.sh tasks               # every task + consensus state
    tools/ccrep_ledger.sh proposals [TASK]    # proposals (all, or one task)
    tools/ccrep_ledger.sh consensus TASK      # full ConsensusState JSON
    tools/ccrep_ledger.sh critiques TASK      # critiques + finding severities
    tools/ccrep_ledger.sh events TASK         # event-log slice

Add ``--json`` to any subcommand for machine-readable output. The ledger path
follows ``CCREP_DB`` (else the canonical shared default) — the same resolution
the MCP server uses, so the inspector and the server always agree.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

_TOOLS_DIR = os.path.dirname(os.path.abspath(__file__))
if _TOOLS_DIR not in sys.path:
    sys.path.insert(0, _TOOLS_DIR)

from ccrep import ledger as ledger_mod  # noqa: E402
from ccrep.reducer import reduce_task  # noqa: E402


def _open_ledger() -> ledger_mod.Ledger:
    path = ledger_mod.db_path()
    if not Path(path).exists():
        sys.exit(f"ccrep-ledger: no ledger at {path}")
    return ledger_mod.Ledger(path)


def _reduce(led: ledger_mod.Ledger, task_id: str):
    events = led.events(task_id)
    if not events:
        sys.exit(f"ccrep-ledger: no events for task {task_id!r}")
    return reduce_task(events)


def _emit(obj, as_json: bool, human) -> None:
    if as_json:
        print(json.dumps(obj, indent=2, default=str))
    else:
        human(obj)


def cmd_db(args, led: ledger_mod.Ledger) -> None:
    print(ledger_mod.db_path())


def cmd_tasks(args, led: ledger_mod.Ledger) -> None:
    rows = []
    for task_id in led.task_ids():
        red = reduce_task(led.events(task_id))
        rows.append(
            {
                "task_id": task_id,
                "state": red.consensus.get("state"),
                "mergeable": red.consensus.get("decision", {}).get("mergeable"),
                "proposals": len(red.proposals),
            }
        )

    def human(rs):
        if not rs:
            print("(no tasks)")
            return
        for r in rs:
            print(
                f"{r['task_id']:<34} {str(r['state']):<18} "
                f"mergeable={r['mergeable']!s:<5} proposals={r['proposals']}"
            )

    _emit(rows, args.json, human)


def _proposal_rows(red) -> list[dict]:
    rows = []
    for pid, ps in sorted(red.proposals.items(), key=lambda kv: kv[1].revision):
        rows.append(
            {
                "proposal_id": pid,
                "revision": ps.revision,
                "author": ps.author,
                "commit_sha": ps.commit_sha,
                "status": ps.proposal.get("status"),
                "merged": ps.merged is not None,
            }
        )
    return rows


def cmd_proposals(args, led: ledger_mod.Ledger) -> None:
    task_ids = [args.task_id] if args.task_id else led.task_ids()
    rows = []
    for tid in task_ids:
        events = led.events(tid)
        if not events:
            continue
        for r in _proposal_rows(reduce_task(events)):
            r["task_id"] = tid
            rows.append(r)

    def human(rs):
        if not rs:
            print("(no proposals)")
            return
        for r in rs:
            print(
                f"{r['proposal_id']}  rev{r['revision']}  {r['author']:<8} "
                f"{r['commit_sha'][:8]}  {str(r['status']):<12} "
                f"{'MERGED' if r['merged'] else ''}  [{r['task_id']}]"
            )

    _emit(rows, args.json, human)


def cmd_consensus(args, led: ledger_mod.Ledger) -> None:
    red = _reduce(led, args.task_id)
    _emit(red.consensus, args.json, lambda c: print(json.dumps(c, indent=2, default=str)))


def cmd_critiques(args, led: ledger_mod.Ledger) -> None:
    red = _reduce(led, args.task_id)
    rows = []
    for cr in red.snapshot.critiques:
        rows.append(
            {
                "critique_id": cr.get("critique_id"),
                "reviewer": cr.get("reviewer"),
                "stance": cr.get("stance"),
                "findings": [
                    {
                        "id": f.get("finding_id"),
                        "severity": f.get("severity"),
                        "category": f.get("category"),
                    }
                    for f in cr.get("findings", [])
                ],
            }
        )

    def human(rs):
        if not rs:
            print("(no critiques)")
            return
        for r in rs:
            sev = ", ".join(
                f"{f['severity']}:{f['id']}" for f in r["findings"]
            ) or "(no findings)"
            print(f"{r['reviewer']:<8} {str(r['stance']):<16} {sev}")

    _emit(rows, args.json, human)


def cmd_events(args, led: ledger_mod.Ledger) -> None:
    events = led.events(args.task_id)
    if not events:
        sys.exit(f"ccrep-ledger: no events for task {args.task_id!r}")
    if args.json:
        print(json.dumps(events, indent=2, default=str))
        return
    for e in events:
        print(f"#{e['seq']:<4} {e['kind']:<22} {str(e['actor']):<8} {e['ts']}")


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="ccrep-ledger", description=__doc__)
    # `--json` lives on every subcommand (via this shared parent) so it works in
    # the natural position, after the subcommand: `... consensus TASK --json`.
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument(
        "--json", action="store_true", help="machine-readable JSON output"
    )
    sub = p.add_subparsers(dest="command", required=True)

    sub.add_parser("db", parents=[common], help="print the resolved canonical ledger path")
    sub.add_parser("tasks", parents=[common], help="list every task and its consensus state")

    pp = sub.add_parser("proposals", parents=[common], help="list proposals (all tasks, or one)")
    pp.add_argument("task_id", nargs="?", default=None)

    for name, helptext in (
        ("consensus", "print the folded ConsensusState for a task"),
        ("critiques", "list critiques + finding severities for a task"),
        ("events", "dump the event-log slice for a task"),
    ):
        sp = sub.add_parser(name, parents=[common], help=helptext)
        sp.add_argument("task_id")

    return p


_DISPATCH = {
    "db": cmd_db,
    "tasks": cmd_tasks,
    "proposals": cmd_proposals,
    "consensus": cmd_consensus,
    "critiques": cmd_critiques,
    "events": cmd_events,
}


def main(argv: list[str] | None = None) -> None:
    args = build_parser().parse_args(argv)
    # `db` does not need an existing ledger to report the path.
    if args.command == "db":
        cmd_db(args, None)  # type: ignore[arg-type]
        return
    led = _open_ledger()
    try:
        _DISPATCH[args.command](args, led)
    finally:
        led.close()


if __name__ == "__main__":
    main()
