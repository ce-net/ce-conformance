// ce-conformance TypeScript/JavaScript runner — drives the @ce-net/sdk (ce-ts) client through
// the shared Tier-A scenario contract (see ../../SCENARIOS.md) against the live node at
// $CE_NODE_URL, using ONLY the SDK's public API. Emits one machine-readable line per scenario:
//
//     CONF <scenario_id> PASS
//     CONF <scenario_id> FAIL: <detail>
//
// and exits non-zero if any scenario fails. Imports the built SDK bundle directly (ce-ts ships a
// dependency-free ESM dist), so no npm install is needed; run with `node run.mjs`.
import {
  CeClient, serve, utf8ToBytes, bytesToUtf8, discoverApiToken,
} from "../../../ce-ts/dist/index.js";

const base = process.env.CE_NODE_URL || "http://127.0.0.1:8844";
const token = process.env.CE_API_TOKEN || (await discoverApiToken());
// maxRetries: 0 so behaviour matches the Go/Python runners (which do not retry) — otherwise the
// SDK's retry/backoff inflates the bounded-error timing of request_unknown_errors.
const ce = new CeClient({ baseUrl: base, token, maxRetries: 0 });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const nonce = () => `${process.pid}-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
const eqBytes = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

const results = [];
const pass = (id) => results.push([id, true, ""]);
const fail = (id, d) => results.push([id, false, String(d)]);

// Subscribe to a fresh topic, publish, and resolve the first payload received on that topic.
async function awaitPublish(topic, payload, timeoutMs = 8000) {
  const ac = new AbortController();
  await ce.mesh.subscribe(topic);
  const got = (async () => {
    for await (const m of ce.mesh.streamMessages({ signal: ac.signal })) {
      if (m.topic === topic) return m.payload();
    }
    throw new Error("stream ended before message");
  })();
  got.catch(() => {}); // avoid an unhandledRejection when we abort after a timeout win
  await sleep(600); // let the subscribe register and the stream establish
  await ce.mesh.publish(topic, payload);
  let timer;
  const timeout = new Promise((_, rej) => {
    timer = setTimeout(() => rej(new Error(`timeout on ${topic}`)), timeoutMs);
  });
  try {
    return await Promise.race([got, timeout]);
  } finally {
    clearTimeout(timer);
    ac.abort();
  }
}

async function main() {
  const self = (await ce.status.status()).nodeId;

  // status
  try {
    const s = await ce.status.status();
    if (s.nodeId) pass("status");
    else fail("status", "empty node_id");
  } catch (e) { fail("status", e); }

  // pubsub_text
  try {
    const got = await awaitPublish(`conf/pubsub-text/${nonce()}`, utf8ToBytes("hello-conformance"));
    if (bytesToUtf8(got) === "hello-conformance") pass("pubsub_text");
    else fail("pubsub_text", `got ${bytesToUtf8(got)}`);
  } catch (e) { fail("pubsub_text", e); }

  // binary_payload
  try {
    const want = new Uint8Array([0, 255, 16, 127, 0, 171]);
    const got = await awaitPublish(`conf/pubsub-bin/${nonce()}`, want);
    if (eqBytes(got, want)) pass("binary_payload");
    else fail("binary_payload", `got ${Array.from(got)}`);
  } catch (e) { fail("binary_payload", e); }

  // request_reply
  try {
    const topic = `conf/reqrep/${nonce()}`;
    const ac = new AbortController();
    const sp = serve(ce, [topic], (req) => utf8ToBytes("echo:" + bytesToUtf8(req.payload)),
      { signal: ac.signal, onWarn: (m, d) => console.error("serve warn:", m, d ?? "") });
    sp.catch(() => {});
    await sleep(1500); // give the responder time to subscribe + open its inbound stream
    const out = await ce.mesh.request(self, topic, utf8ToBytes("ping"), 8000);
    ac.abort();
    if (bytesToUtf8(out) === "echo:ping") pass("request_reply");
    else fail("request_reply", `got ${bytesToUtf8(out)}`);
  } catch (e) { fail("request_reply", e); }

  // request_unknown_errors
  try {
    const start = Date.now();
    try {
      await ce.mesh.request("0".repeat(64), `conf/nonexistent/${nonce()}`, utf8ToBytes("x"), 3000);
      fail("request_unknown_errors", "expected error, got success");
    } catch {
      const el = Date.now() - start;
      if (el < 9000) pass("request_unknown_errors");
      else fail("request_unknown_errors", `unbounded: ${el}ms`);
    }
  } catch (e) { fail("request_unknown_errors", e); }

  let allPass = true;
  for (const [id, okv, d] of results) {
    if (okv) console.log(`CONF ${id} PASS`);
    else { allPass = false; console.log(`CONF ${id} FAIL: ${d}`); }
  }
  process.exit(allPass ? 0 : 1);
}

main().catch((e) => { console.log(`CONF setup FAIL: ${e}`); process.exit(2); });
