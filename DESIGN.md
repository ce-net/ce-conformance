# ce-conformance — design, decisions & how to extend

The [README](README.md) is how to run it; [SCENARIOS.md](SCENARIOS.md) is the contract. This is the
"why," plus the contributor guide (add a language, add a scenario) — so this repo is self-contained.

## The problem it solves

CE has one product — the node client — in many languages (`ce-rs`, `ce-ts`, `ce-py`, `ce-go`, and
whatever comes next). With N independent implementations, the failure mode is **drift**: SDK X
behaves one way, SDK Y another, or the node changes and only one SDK is updated. Nobody notices until
an app that worked in Python breaks in Go.

This kit makes conformance **testable and mechanical**: a shared, language-neutral behavioral contract
+ one thin runner per implementation that drives that implementation's public API against a live node
+ a driver that runs every runner and prints a cross-language pass/fail matrix. A language is
conformant when its runner is green on every scenario. Drift becomes a failing test.

## Decisions (and the reasoning)

**Behavioral equivalence, not shared code / codegen.** The SDKs are hand-written to be idiomatic (Go
`(T, error)` + `ctx`; Python generators; TS async iterables; Rust `Result`). We verify they *behave*
the same, we do not force them to *be* the same. A transpiled lowest-common-denominator client would
be unidiomatic and unadoptable — the opposite of "distributed systems made easy." The kit buys
cross-language correctness without a shared implementation.

**A runnable contract, not prose.** `SCENARIOS.md` describes behavior in English, but the truth is the
runners — they execute. "Conformant" is a green run, not a promise.

**Pinned cross-implementation invariants.** The strongest scenario is `object_cid`: every SDK must
reproduce a hardcoded object CID for a fixed 256-byte input. That proves the 1 MiB chunking + manifest
bytes are byte-identical across languages — the content-address portability the whole p2p-distribution
story rides on. Where two implementations *must* agree exactly, pin the value.

**Economy-agnostic core.** The substrate contract must not depend on optional layers. The core node is
chain-free; economic endpoints 503. So scenarios test the mesh/substrate surface and treat economy as
optional (`economy_gated` asserts the *gating* is correct, not that economy exists).

**One node, self-loopback (for now).** The kit drives a single live node that delivers its own
published/directed messages to its own subscribers (verified). This exercises the full local
publish→stream and request→reply paths. True multi-node topologies (cross-node fan-out, NAT, relay)
are the `ce-test` framework's job (a separate workstream) — this kit provides the *scenarios*; that
framework provides the *topology*. Keep them decoupled.

**Portable driver.** `run.sh` is bash-3.2 compatible (macOS stock bash — no associative arrays),
auto-skips absent toolchains, builds ce-ts's dist if missing, compiles ce-rs on first run, and gates
CI on any FAIL.

## The runner contract (all a new language must obey)

A runner uses ONLY its SDK's public API to execute each scenario in `SCENARIOS.md`, then prints to
stdout, one line per scenario:

```
CONF <scenario_id> PASS
CONF <scenario_id> FAIL: <detail>
```

and exits `0` iff all passed (`2` reserved for setup failure, e.g. no node). That is the entire
contract between a runner and the driver — so runners stay tiny and each SDK remains the source of
truth for its own behavior.

Conventions every runner MUST follow:
- **Unique topics** per run: `conf/<name>/<nonce>` — isolates a scenario from other runs and from the
  node's background mesh traffic.
- **Public API only** — no raw HTTP, no reaching around the SDK. The point is to test the SDK.
- **Bounded waits** — every receive/await has a timeout; a would-block scenario is a FAIL, never a hang.

## How to add a language (the headline extension)

1. Write a thin client SDK for the language (start from `ce-py` — the smallest complete reference).
2. Create `runners/<lang>/` that drives it through every scenario in `SCENARIOS.md` and emits the
   `CONF` lines. Copy the shape of an existing runner in the closest paradigm (`runners/go/main.go`
   for typed/compiled, `runners/python/run.py` for scripting).
3. Wire it into `run.sh`: add a `run_lang <lang> "<command>"` guarded by `command -v <toolchain>`.
4. Make it green: `./run.sh`. That's "done."

Ordering is demand-driven: C#/.NET, JVM (Java/Kotlin), Swift (Apple-native), Ruby, PHP, C/C++
(embedded/FFI), Elixir (actor-model fit for `serve`).

## How to add a scenario

1. Add it to `SCENARIOS.md` (the contract) — id, what it asserts, any pinned constant.
2. Implement the same id in **every** runner (same behavior, same id).
3. Add the id to the `SCENARIOS` list in `run.sh`.
Keep it economy-agnostic where possible; anything with a pinnable cross-SDK invariant (like
`object_cid`) is high value. Candidates: jobs lifecycle, signals, block/tx streams, caps/tags.

## Proof it earns its keep

Adding the TypeScript runner immediately caught a real interop bug — ce-ts rounded the node's u64
`reply_token` (> 2^53) via `JSON.parse`, silently breaking request/reply, while Go/Python/Rust parsed
it losslessly. Adding the Tier-B `economy_gated` scenario caught a second — ce-ts's `NodeStatus`
omitted the `economy` flag the other three surfaced. Both were fixed. That is the entire value: a
cross-language bug becomes a red cell instead of a production surprise.

## Layout

```
SCENARIOS.md   the language-neutral behavioral contract (source of truth)
run.sh         the driver (bash 3.2 compatible)
runners/go/    Go runner     — drives ce-go
runners/python Python runner — drives ce.py (via $CE_PY_DIR)
runners/ts/    TS/JS runner  — drives @ce-net/sdk (imports ../../../ce-ts/dist)
runners/rust/  Rust runner   — drives ce-rs (path dep), the reference SDK
```

`runners/rust/.cargo/config.toml` is generated by `tools/ce-dev-link` (machine-specific paths) and is
git-ignored — never commit it.
