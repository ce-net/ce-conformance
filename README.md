# ce-conformance — the CE SDK conformance kit

One behavioral test suite that every CE SDK, in every language, must pass — the mechanism that
keeps the polyglot SDK family (`ce-rs`, `ce-ts`, `ce-py`, `ce-go`, …) honest as it grows.

The problem it solves: with N language SDKs, the failure mode is drift — SDK X behaves one way,
SDK Y another, or the node changes and only one SDK is updated. This kit makes conformance
**testable and mechanical**: a shared, language-neutral scenario contract ([`SCENARIOS.md`](SCENARIOS.md))
+ one thin runner per language that drives that language's SDK through the scenarios against a
live node + a driver that runs every runner and prints a cross-language pass/fail matrix.

**"Prove scalability" becomes a checkmark, not a code review.** Adding a language to CE is: write
a runner in `runners/<lang>/` that speaks the `CONF` output contract, and make it green here.

## Run it

Needs one live CE node (any node — the kit tests the economy-agnostic mesh surface, so it does
not matter whether an economy adapter is attached):

```sh
ce start            # in another terminal, if you don't already have a node
./run.sh            # runs every available SDK runner against http://127.0.0.1:8844
CE_NODE_URL=http://host:8844 ./run.sh    # target a specific node
```

Example output:

```
=== conformance matrix ===
scenario                  go        python
status                    ok        ok
pubsub_text               ok        ok
binary_payload            ok        ok
request_reply             ok        ok
request_unknown_errors    ok        ok

RESULT: PASS (all runners conformant)
```

The driver auto-skips a language whose toolchain is absent, and exits non-zero on any FAIL.

## Layout

```
SCENARIOS.md         the language-neutral behavioral contract (the source of truth)
run.sh               the driver: run every runner, build the matrix, gate on failures
runners/go/          Go runner   — drives ce-go   (github.com/ce-net/ce-go)
runners/python/      Python runner — drives ce.py (../ce-py, via $CE_PY_DIR)
```

Runners to add next (mechanical — same scenarios, same `CONF` lines): `runners/ts/` (`@ce-net/sdk`),
`runners/rust/` (`ce-rs`, the reference). Then the whole matrix widens as new languages arrive.

## How a runner works

A runner uses ONLY its SDK's public API to execute each scenario, then prints:

```
CONF <scenario_id> PASS
CONF <scenario_id> FAIL: <detail>
```

exiting `0` iff all passed. That is the entire contract between a runner and the driver — so
runners stay tiny and each SDK remains the source of truth for its own behavior.

## Where this is going

This is the seed of the wider CE testing framework (`PLAN/ce-testing-framework.md`): it will
grow to boot ephemeral nodes itself (including ≥2-node topologies for true cross-node pubsub),
and to a Tier-B suite (jobs/signals/streams/wallet/blobs, economy-optional). For now it runs
against a node you already have, which is enough to keep the SDK family in lockstep.

Design + strategy: `PLAN/ce-polyglot-sdks.md`.
