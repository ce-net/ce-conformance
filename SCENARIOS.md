# The CE SDK conformance contract (Tier A)

This is the language-neutral behavioral contract every CE SDK must satisfy. Each scenario is
implemented identically by every language runner (`runners/<lang>/`) using ONLY that SDK's
public API, and asserted against one live node. A language is **conformant** when its runner
passes all of these. Adding a language to CE is: write a runner that passes this list.

These scenarios cover the **economy-agnostic app / mesh surface** on purpose. The substrate's
economy is being extracted into an adapter and will eventually leave the substrate entirely
(the `--no-economy` flag is transitional and will be removed), so the core SDK contract must
never depend on economy being present. Economy/jobs/blobs are a later, separate tier of the kit.

## Output contract

Each runner prints, to stdout, one line per scenario:

```
CONF <scenario_id> PASS
CONF <scenario_id> FAIL: <detail>
```

and exits `0` iff every scenario passed (`2` reserved for setup failure, e.g. no node). The
driver `run.sh` parses these lines into a cross-language matrix and returns non-zero on any FAIL.

## Scenarios

| id | asserts |
|---|---|
| `status` | `Status()` returns a non-empty `node_id`. Liveness + response shape. |
| `pubsub_text` | Subscribe to a fresh unique topic, publish a UTF-8 payload, and receive it on that topic — payload intact. Exercises subscribe + publish + the SSE inbound stream. |
| `binary_payload` | Same round-trip with a non-UTF-8 binary payload (`00 ff 10 7f 00 ab`), asserted byte-exact. Proves the hex wire encoding, not text mangling. |
| `request_reply` | A `Serve` responder that echoes answers a directed `Request` end to end; the reply equals `echo:<payload>`. Exercises the full request/reply loop. |
| `request_unknown_errors` | A request to an unreachable node id (`00…00`) with a short timeout raises a typed error within its bound — it neither hangs nor reports false success. Proves error propagation + timeout bounding. |

### Conventions every runner MUST follow

- **Unique topics.** Each scenario uses a per-run-unique topic (`conf/<name>/<nonce>`) so it is
  isolated from other runs and from unrelated background mesh traffic on the shared node.
- **Public API only.** No raw HTTP, no reaching around the SDK — the point is to test the SDK.
- **Bounded waits.** Every receive/await has a timeout; a scenario that would block forever is a
  FAIL, never a hang.

## Not yet covered (future tiers)

- **Multi-node pubsub fan-out** — true cross-node delivery needs the kit to boot ≥2 nodes; a hook
  for the ce testing framework. The single-node scenarios above already exercise the full local
  publish→stream and request→reply paths (a node delivers its own published/directed messages to
  its local subscribers, verified live).
- **Tier B**: jobs, signals, streams, wallet/caps/tags, blobs (`data`), and the OPTIONAL economy
  (must tolerate its absence). Added as `runners/*/` grow to full-node parity.
