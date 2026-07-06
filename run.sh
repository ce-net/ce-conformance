#!/usr/bin/env bash
# ce-conformance driver — run every available SDK's conformance runner against ONE live CE
# node and print a cross-language pass/fail matrix.
#
# A language SDK is "conformant" when its runner passes every scenario in SCENARIOS.md. Adding
# a language to CE = write a runner (runners/<lang>/) that speaks the CONF output contract and
# make it green here. This is how "prove scalability" becomes a checkmark instead of a review.
#
# It tests the economy-AGNOSTIC mesh surface only, so it does not care whether the node has an
# economy adapter attached. Usage: ./run.sh   (override the node with CE_NODE_URL=...).
#
# Portable to macOS's stock bash 3.2 — no associative arrays.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
WS="$(cd "$HERE/.." && pwd)"
NODE="${CE_NODE_URL:-http://127.0.0.1:8844}"
TMP="$(mktemp -d 2>/dev/null || echo /tmp/ce-conf-$$)"; mkdir -p "$TMP"

if ! curl -s -m 3 "$NODE/status" >/dev/null 2>&1; then
  echo "No CE node at $NODE. Start one (e.g. 'ce start') and retry." >&2
  echo "The kit drives every SDK against ONE live node over the mesh surface." >&2
  exit 2
fi

SCENARIOS="status pubsub_text binary_payload request_reply request_unknown_errors blob_roundtrip object_roundtrip object_cid amount_wire economy_gated"
LANGS=""
FAILED=0

run_lang() {
  lang="$1"; cmd="$2"
  bash -c "$cmd" >"$TMP/$lang.out" 2>"$TMP/$lang.err"
  code=$?
  printf '\n=== %s runner (exit %s) ===\n' "$lang" "$code"
  cat "$TMP/$lang.out"
  [ -s "$TMP/$lang.err" ] && { printf '(stderr)\n'; cat "$TMP/$lang.err"; }
  LANGS="$LANGS $lang"
  [ "$code" -ne 0 ] && FAILED=1
  return 0
}

# cell <lang> <scenario> -> prints ok / FAIL / --
cell() {
  f="$TMP/$1.out"
  [ -f "$f" ] || { printf -- '--'; return; }
  if grep -q "^CONF $2 PASS" "$f"; then printf 'ok'
  elif grep -q "^CONF $2 FAIL" "$f"; then printf 'FAIL'
  else printf -- '--'; fi
}

echo "conformance node: $NODE"

if command -v go >/dev/null 2>&1; then
  run_lang go "cd '$HERE/runners/go' && CE_NODE_URL='$NODE' go run ."
else
  echo "(skip go: toolchain not found)"
fi

if command -v python3 >/dev/null 2>&1; then
  run_lang python "CE_NODE_URL='$NODE' CE_PY_DIR='$WS/ce-py' python3 '$HERE/runners/python/run.py'"
else
  echo "(skip python: python3 not found)"
fi

if command -v node >/dev/null 2>&1; then
  # ce-ts ships a dependency-free ESM dist the runner imports directly; build it once if absent.
  if [ ! -f "$WS/ce-ts/dist/index.js" ] && command -v npm >/dev/null 2>&1; then
    echo "building ce-ts dist..."; ( cd "$WS/ce-ts" && npm run build >/dev/null 2>&1 )
  fi
  if [ -f "$WS/ce-ts/dist/index.js" ]; then
    run_lang ts "CE_NODE_URL='$NODE' node '$HERE/runners/ts/run.mjs'"
  else
    echo "(skip ts: ce-ts/dist not built — run 'npm run build' in ce-ts)"
  fi
else
  echo "(skip ts: node not found)"
fi

if command -v cargo >/dev/null 2>&1; then
  # First run compiles ce-rs (~1-2 min); subsequent runs are instant.
  run_lang rust "cd '$HERE/runners/rust' && CE_NODE_URL='$NODE' cargo run -q"
else
  echo "(skip rust: cargo not found)"
fi

printf '\n=== conformance matrix ===\n'
printf '%-26s' "scenario"
for l in $LANGS; do printf '%-10s' "$l"; done
printf '\n'
for sc in $SCENARIOS; do
  printf '%-26s' "$sc"
  for l in $LANGS; do printf '%-10s' "$(cell "$l" "$sc")"; done
  printf '\n'
done

printf '\n'
if [ "$FAILED" -ne 0 ]; then
  echo "RESULT: FAIL"
else
  echo "RESULT: PASS (all runners conformant)"
fi
exit "$FAILED"
