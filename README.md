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

Rationale, the contributor guide (add a language / add a scenario), and the design decisions are in
**[DESIGN.md](DESIGN.md)**; the behavioral contract itself is **[SCENARIOS.md](SCENARIOS.md)**.

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
scenario                  go        python    ts        rust
status                    ok        ok        ok        ok
pubsub_text               ok        ok        ok        ok
binary_payload            ok        ok        ok        ok
request_reply             ok        ok        ok        ok
request_unknown_errors    ok        ok        ok        ok
blob_roundtrip            ok        ok        ok        ok
object_roundtrip          ok        ok        ok        ok
object_cid                ok        ok        ok        ok
amount_wire               ok        ok        ok        ok
economy_gated             ok        ok        ok        ok

RESULT: PASS (all runners conformant)
```

Both tiers run: **Tier A** (the app/mesh surface) and **Tier B** (money, blobs, content-addressed
objects, economy). See [`SCENARIOS.md`](SCENARIOS.md).

The driver auto-skips a language whose toolchain is absent, and exits non-zero on any FAIL. (The
Rust runner compiles ce-rs on first run, ~1-2 min; the TS runner builds ce-ts's dist once if absent.)

## Layout

```
SCENARIOS.md         the language-neutral behavioral contract (the source of truth)
run.sh               the driver: run every runner, build the matrix, gate on failures
runners/go/          Go runner     — drives ce-go
runners/python/      Python runner — drives ce.py
runners/ts/          TS/JS runner  — drives @ce-net/sdk
runners/rust/        Rust runner   — drives ce-rs, the reference SDK
```

All four current CE SDKs are covered. Adding the next language is mechanical: a new `runners/<lang>/`
that speaks the `CONF` output contract, made green here.

## What the kit is for — a worked example

The kit is not ceremony: adding the TS runner immediately surfaced a real interop bug. The node's
`reply_token` is a u64 that can exceed JS's 2^53 safe integer; ce-ts parsed it with `JSON.parse`,
rounding it, so `mesh.reply()` sent the wrong token and every request timed out — while Go (`uint64`)
and Python (bigint) parsed it losslessly and passed. The red cell drove the fix (ce-ts now carries
`replyToken` as a lossless string). Adding the Tier-B `economy_gated` scenario then caught a second
one — ce-ts's `NodeStatus` didn't expose the node's `economy` flag that Go/Python/Rust all surface
(fixed). That is the whole point: cross-language drift becomes a failing test, not a latent
production bug.

## How a runner works

A runner uses ONLY its SDK's public API to execute each scenario, then prints:

```
CONF <scenario_id> PASS
CONF <scenario_id> FAIL: <detail>
```

exiting `0` iff all passed. That is the entire contract between a runner and the driver — so
runners stay tiny and each SDK remains the source of truth for its own behavior.

## Where this is going

This is a seed for a wider distributed test framework: it will grow to boot ephemeral nodes
itself (including ≥2-node topologies for true cross-node pubsub) and to a fuller Tier-B suite
(jobs/signals/streams). For now it runs against a node you already have, which is enough to keep
the SDK family in lockstep. Design, decisions, and the contributor guide: [DESIGN.md](DESIGN.md).
