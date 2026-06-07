"""Standalone invariant tests for MailStore (no MCP runtime needed).

Run from the tools/ directory:
    tools/.venv/bin/python -m collab_mcp.test_store
"""

from __future__ import annotations

import json
import tempfile
import time
import uuid

from .store import MailStore


def main() -> None:
    root = tempfile.mkdtemp(prefix="collab_test_")
    claude = MailStore(root, "claude")
    agy = MailStore(root, "agy")

    # mailbox: send -> recv, message archived to cur/
    sent = claude.send("agy", "hello", {"text": "hi"})
    got = agy.recv("hello", timeout_s=2)
    assert got is not None and got["id"] == sent["id"], "recv returns the sent message"
    assert got["payload"]["text"] == "hi"
    assert agy.try_recv() is None, "new/ empty after recv"
    assert list((agy.root / "agy" / "cur").glob("*.json")), "message archived in cur/"

    # selective receive by type (leave non-matching in place)
    claude.send("agy", "typeA", 1)
    claude.send("agy", "typeB", 2)
    b = agy.recv("typeB", timeout_s=2)
    assert b is not None and b["type"] == "typeB" and b["payload"] == 2, "selective receive"
    a = agy.try_recv("typeA")
    assert a is not None and a["type"] == "typeA", "the other message was left for later"

    # message order comes from envelope ts, not adapter-specific filenames
    early = {"id": uuid.uuid4().hex, "from": "claude", "to": "agy", "type": "ordered", "payload": "early", "ts": 1.0}
    late = {"id": uuid.uuid4().hex, "from": "claude", "to": "agy", "type": "ordered", "payload": "late", "ts": 2.0}
    (agy.root / "agy" / "new" / f"z-foreign-{early['id']}.json").write_text(json.dumps(early))
    (agy.root / "agy" / "new" / f"a-foreign-{late['id']}.json").write_text(json.dumps(late))
    assert agy.try_recv("ordered")["id"] == early["id"], "recv order follows envelope ts"
    assert agy.try_recv("ordered")["id"] == late["id"], "filename is only a tie-breaker"

    # recv blocks then times out -> None
    t0 = time.monotonic()
    assert agy.recv("nope", timeout_s=0.5) is None, "recv times out to None"
    assert time.monotonic() - t0 >= 0.4, "recv actually waited for the timeout"

    # request/response correlation
    req = claude.send("agy", "ask", {"q": "ready?"})
    reply = agy.send("claude", "ans", {"a": "yes"}, in_reply_to=req["id"])
    assert reply["in_reply_to"] == req["id"], "in_reply_to threads the reply"
    assert claude.recv("ans", timeout_s=2)["in_reply_to"] == req["id"]

    # task queue: post / claim (exactly one) / complete
    w1 = MailStore(root, "w1")
    w2 = MailStore(root, "w2")
    task = claude.post({"job": "build"})
    c1 = w1.claim()
    c2 = w2.claim()
    assert c1 is not None and c1["id"] == task["id"], "w1 claims the task"
    assert c2 is None, "w2 must NOT claim the same task (atomic claim)"
    done = w1.complete(task["id"], {"branch": "feat/x"})
    assert done is not None and done["status"] == "done", "complete marks done"
    assert done["result"]["branch"] == "feat/x", "result is recorded"
    assert any((claude.root / "tasks" / "done").glob("*.json")), "task moved to done/"
    assert w1.claim() is None, "no open tasks remain"

    # lease: claim stamps an expiry; heartbeat extends it
    leased = claude.post({"job": "leased"})
    held = w1.claim(lease_ttl=100.0)
    assert held["id"] == leased["id"] and held["lease_expires_at"] > time.time(), "claim sets a lease"
    before = held["lease_expires_at"]
    time.sleep(0.01)
    beat = w1.heartbeat(leased["id"], lease_ttl=200.0)
    assert beat is not None and beat["lease_expires_at"] > before, "heartbeat extends the lease"
    assert w2.heartbeat(leased["id"]) is None, "cannot heartbeat a task you don't hold"

    # abandon: returns the task to open/ with a reason, reclaimable by anyone
    ab = w1.abandon(leased["id"], "bad environment")
    assert ab is not None and ab["status"] == "open" and ab["abandoned_reason"] == "bad environment"
    assert "owner" not in ab and "lease_expires_at" not in ab, "claim fields stripped on abandon"
    re = w2.claim()
    assert re is not None and re["id"] == leased["id"], "abandoned task is re-claimable"
    w2.complete(leased["id"], {"ok": True})

    # sweep: a coordinator reclaims an expired lease across actors
    swept_task = claude.post({"job": "will-expire"})
    w1.claim(lease_ttl=0.01)  # tiny lease
    time.sleep(0.02)  # let it expire
    coordinator = MailStore(root, "coordinator")
    reclaimed = coordinator.sweep()
    assert any(r["id"] == swept_task["id"] for r in reclaimed), "sweep reclaims expired lease"
    assert reclaimed[0]["swept_from"] == "w1", "sweep records the previous owner"
    assert w2.claim()["id"] == swept_task["id"], "reclaimed task is back in open/"
    w2.complete(swept_task["id"], {"ok": True})

    # cross-scheme filename: complete locates a claimed task by env id, not filename
    foreign = {"id": uuid.uuid4().hex, "from": "agy", "type": "task", "payload": {}, "status": "open"}
    (claude.root / "tasks" / "open" / f"foreign-prefix-{foreign['id']}.json").write_text(json.dumps(foreign))
    fc = w1.claim()
    assert fc is not None and fc["id"] == foreign["id"], "claims a foreign-named task file"
    fdone = w1.complete(foreign["id"], {"ok": True})
    assert fdone is not None and fdone["status"] == "done", "completes by env id despite odd filename"

    # tracing snapshot
    snap = claude.inbox("agy")
    assert snap["actor"] == "agy" and "new" in snap and "tasks_done" in snap, "inbox snapshot shape"

    print("OK - all collab store invariants hold")


if __name__ == "__main__":
    main()
