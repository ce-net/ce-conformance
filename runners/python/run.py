#!/usr/bin/env python3
"""ce-conformance Python runner — drives the ce.py SDK through the shared Tier-A scenario
contract (see ../../SCENARIOS.md) against the live node at $CE_NODE_URL, using ONLY the SDK's
public API. It emits one machine-readable line per scenario:

    CONF <scenario_id> PASS
    CONF <scenario_id> FAIL: <detail>

and exits non-zero if any scenario fails. Every language runner implements the same scenarios
with the same ids and output contract, so the driver (../../run.sh) builds one cross-language
matrix. Point $CE_PY_DIR at the ce-py repo so `import ce` finds the vendored client.
"""

import os
import queue
import sys
import threading
import time

sys.path.insert(0, os.environ.get("CE_PY_DIR", ""))
import ce  # noqa: E402  (path is set above)


def nonce() -> str:
    return f"{os.getpid()}-{time.time_ns()}"


def await_publish(c: "ce.Client", topic: str, payload: bytes, timeout: float = 8.0) -> bytes:
    """Subscribe to topic, publish payload, return the first payload received on that topic."""
    q: "queue.Queue" = queue.Queue()

    def reader():
        try:
            for m in c.subscribe(topic):  # yields only this topic
                q.put(m.payload)
                return
        except Exception as e:  # surface the error to the waiter
            q.put(e)

    threading.Thread(target=reader, daemon=True).start()
    time.sleep(0.6)  # let the subscribe register and the stream establish
    c.publish(topic, payload)
    try:
        r = q.get(timeout=timeout)
    except queue.Empty:
        raise TimeoutError(f"timeout: no message on {topic}")
    if isinstance(r, Exception):
        raise r
    return r


def s1_status(c) -> tuple[bool, str]:
    s = c.status()
    if not s.get("node_id"):
        return False, "empty node_id"
    return True, ""


def s2_pubsub_text(c) -> tuple[bool, str]:
    topic = "conf/pubsub-text/" + nonce()
    want = b"hello-conformance"
    got = await_publish(c, topic, want)
    return (got == want), f"got {got!r} want {want!r}"


def s3_binary_payload(c) -> tuple[bool, str]:
    topic = "conf/pubsub-bin/" + nonce()
    want = bytes([0x00, 0xFF, 0x10, 0x7F, 0x00, 0xAB])
    got = await_publish(c, topic, want)
    return (got == want), f"got {got!r} want {want!r}"


def s4_request_reply(c, self_id: str) -> tuple[bool, str]:
    topic = "conf/reqrep/" + nonce()

    def handler(m):
        return b"echo:" + m.payload

    threading.Thread(target=lambda: c.serve([topic], handler), daemon=True).start()
    time.sleep(0.6)  # let the responder subscribe
    out = c.request(self_id, topic, b"ping", 8000)
    return (out == b"echo:ping"), f"got {out!r}"


def s5_request_unknown_errors(c) -> tuple[bool, str]:
    bogus = "0" * 64
    start = time.time()
    try:
        c.request(bogus, "conf/nonexistent/" + nonce(), b"x", 3000)
        return False, "expected error, got success"
    except ce.CeError:
        elapsed = time.time() - start
        return (elapsed < 9.0), f"bounded in {elapsed:.1f}s"


def main() -> int:
    c = ce.connect()
    try:
        c.wait_ready(15.0)
    except ce.CeError as e:
        print(f"CONF setup FAIL: node not ready: {e}")
        return 2
    self_id = c.node_id
    if not self_id:
        print("CONF setup FAIL: no node id")
        return 2

    scenarios = [
        ("status", lambda: s1_status(c)),
        ("pubsub_text", lambda: s2_pubsub_text(c)),
        ("binary_payload", lambda: s3_binary_payload(c)),
        ("request_reply", lambda: s4_request_reply(c, self_id)),
        ("request_unknown_errors", lambda: s5_request_unknown_errors(c)),
    ]

    all_pass = True
    for sid, fn in scenarios:
        try:
            ok, detail = fn()
        except Exception as e:  # a scenario that throws is a failure, not a crash
            ok, detail = False, f"{type(e).__name__}: {e}"
        if ok:
            print(f"CONF {sid} PASS")
        else:
            all_pass = False
            print(f"CONF {sid} FAIL: {detail}")

    return 0 if all_pass else 1


if __name__ == "__main__":
    sys.exit(main())
