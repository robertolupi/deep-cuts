"""Standalone invariant tests for MailStore (no MCP runtime needed).

Run from the tools/ directory:
    tools/.venv/bin/python -m collab_mcp.test_store
"""

from __future__ import annotations

import tempfile
import time

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

    # tracing snapshot
    snap = claude.inbox("agy")
    assert snap["actor"] == "agy" and "new" in snap and "tasks_done" in snap, "inbox snapshot shape"

    print("OK - all collab store invariants hold")


if __name__ == "__main__":
    main()
