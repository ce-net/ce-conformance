---
name: ce-conformance
description: How to run and extend the CE SDK conformance kit — the cross-language behavioral matrix that keeps ce-rs/ce-ts/ce-py/ce-go identical. Read before adding a language, adding a scenario, or debugging SDK drift. Ships with the repo.
---

# ce-conformance — the CE SDK conformance kit

One behavioral test suite every CE SDK, in every language, must pass. It is how the polyglot client
family (`ce-rs`, `ce-ts`, `ce-py`, `ce-go`, …) stays honest: a language-neutral contract + one thin
runner per SDK + a driver that builds a cross-language pass/fail matrix. Drift becomes a failing test.

## Run it

Needs one live CE node (any node — the kit tests the economy-agnostic mesh surface):

```sh
ce start            # in another terminal if you don't have a node
./run.sh            # runs every available SDK runner against http://127.0.0.1:8844, prints a matrix
CE_NODE_URL=http://host:8844 ./run.sh     # target a specific node
```

The driver auto-skips a language whose toolchain is absent, builds ce-ts's dist if missing, compiles
ce-rs on first run (~1-2 min), and exits non-zero on any FAIL. It is bash-3.2 compatible (macOS stock
bash). Run it from a full local checkout with the SDK repos present as siblings (see DESIGN.md
§Self-containment).

## The contract

- `SCENARIOS.md` — the language-neutral behavioral contract (10 scenarios: 5 Tier A + 5 Tier B,
  including `object_cid`, a pinned constant every SDK must reproduce → cross-language content-address
  portability). This is the source of truth.
- `run.sh` — the driver.
- `runners/<lang>/` — one per SDK; each drives ONLY its SDK's public API and prints, per scenario:
  `CONF <id> PASS` or `CONF <id> FAIL: <detail>`, exiting 0 iff all passed. That output line is the
  entire contract between a runner and the driver.

## Add a language

1. Write the SDK (start from `ce-py` — the smallest complete reference client).
2. Create `runners/<lang>/` that drives it through every `SCENARIOS.md` scenario and emits the `CONF`
   lines (copy the closest existing runner — `runners/go` for typed/compiled, `runners/python` for
   scripting).
3. Wire it into `run.sh` (`run_lang <lang> "<cmd>"`, guarded by `command -v`).
4. `./run.sh` green = done.

## Add a scenario

Add it to `SCENARIOS.md`, implement the same id in EVERY runner, add the id to the `SCENARIOS` list in
`run.sh`. Keep it economy-agnostic; anything with a pinnable cross-SDK invariant (like `object_cid`) is
high value. Unique topics per run (`conf/<name>/<nonce>`) to isolate from background mesh traffic.

## Why it exists

Behavioral equivalence without shared code — each SDK stays idiomatic; a green run is the trust, not a
code review. It has already caught two real ce-ts interop bugs (a u64 `reply_token` rounded by JS; a
missing `economy` field). Full rationale + the self-containment TODO: `DESIGN.md`.
