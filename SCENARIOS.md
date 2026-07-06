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

## Tier B — the full node surface

These run right after Tier A in every runner, exercising the full-node SDK surface (money, blobs,
content-addressed objects, economy) that `ce-rs`/`ce-ts`/`ce-go`/`ce-py` all implement.

| id | asserts |
|---|---|
| `blob_roundtrip` | `put_blob(data)` returns a hash equal to the locally computed `cid(data)` (lowercase hex SHA-256), and `get_blob` returns the exact bytes. Proves the SDK's content id matches the node's. |
| `object_roundtrip` | A multi-chunk object (`2·chunkSize + 123` bytes) round-trips byte-exact through `put_object`/`get_object` (1 MiB chunks, per-chunk CID verify). |
| `object_cid` | `put_object` of the **canonical object** — bytes `0x00..0xff` (256 bytes) — returns the pinned CID `6523c7e119dc980a9267de7c59a8e5390c294646a1c7ab28e218de0da0b69994`. Every SDK must match it: this is the cross-language content-address portability guarantee (identical chunking + manifest bytes). |
| `amount_wire` | `parse_credits("1.5")` yields `1500000000000000000` base units, renders back to `"1.5"`, and serializes to the JSON **string** `"1500000000000000000"` — money is integer base units on the wire, never a float or a JSON number. |
| `economy_gated` | A `transfer` to self of 1 credit succeeds iff the node's `status.economy` is enabled, and is refused (a graceful error, never success, never a hang) when it is off. Every SDK must surface the node's economy mode and gate accordingly. |

The canonical object's manifest is the compact JSON
`{"kind":"ce-object-v1","chunk_size":1048576,"total_size":256,"chunks":["<sha256 of 0x00..0xff>"]}`;
the object CID is its SHA-256. Any SDK whose chunking or manifest differs by a byte fails `object_cid`.

## Not yet covered (future tiers)

- **Multi-node pubsub fan-out** — true cross-node delivery needs the kit to boot ≥2 nodes; a hook
  for the ce testing framework. The single-node scenarios above already exercise the full local
  publish→stream and request→reply paths (a node delivers its own published/directed messages to
  its local subscribers, verified live).
- **More Tier B**: jobs lifecycle, signals, block/tx streams, wallet/caps/tags — added as needed.
