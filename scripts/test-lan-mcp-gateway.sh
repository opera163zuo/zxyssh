#!/bin/bash
# End-to-end connectivity check for the LAN MCP gateway (`zxyssh mcp serve`).
#
# Everything runs against a throwaway state directory under target/, so the
# operator's real servers, credentials and paired Agents are never read or
# written. No target SSH host is contacted: the checks cover the HTTP transport,
# pairing, authentication, limits, audit-at-rest and revocation only.
#
#   usage: bash scripts/test-lan-mcp-gateway.sh [interface-address]
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=target/debug/zxyssh
mkdir -p target
WORK=$(mktemp -d "$PWD/target/lan-mcp-gateway.XXXXXX")
chmod 700 "$WORK"
SRV=""
CHECKS=0

cleanup() {
  if [ -n "$SRV" ] && kill -0 "$SRV" 2>/dev/null; then kill -TERM "$SRV" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

for tool in curl python3; do
  command -v "$tool" >/dev/null || { echo "missing dependency: $tool" >&2; exit 1; }
done
[ -x "$BIN" ] || cargo build -p zxyssh --bin zxyssh

# An admin-facing check must not depend on a hand-picked port.
PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')
LAN_IP=${1:-}
if [ -z "$LAN_IP" ]; then
  LAN_IP=$(ip -4 -o addr show scope global | awk '$2!~/^(docker|br-|veth|lo)/ {split($4,a,"/"); print a[1]; exit}')
fi
# Section 1 asserts the guard rejects a non-loopback listener. A loopback
# argument would make that assertion meaningless, so refuse it here instead of
# reporting a confusing failure later.
case "$LAN_IP" in
  127.*|::1|localhost) echo "argument must be a non-loopback address, got $LAN_IP" >&2; exit 2 ;;
esac

start() { # start <state-dir> <bind> [allow-lan]
  local state=$1 bind=$2 lan=${3:-false}
  XDG_DATA_HOME="$state" ZXYSSH_MCP_BIND="$bind" ZXYSSH_MCP_ALLOW_LAN="$lan" \
    "$BIN" mcp serve > "$state/serve.log" 2>&1 &
  SRV=$!
  for _ in $(seq 1 40); do
    curl -fs -m 1 "http://$bind/health" >/dev/null 2>&1 && return 0
    kill -0 "$SRV" 2>/dev/null || return 1
    sleep 0.25
  done
  return 1
}

stop() {
  if [ -n "$SRV" ]; then kill -TERM "$SRV" 2>/dev/null || true; wait "$SRV" 2>/dev/null || true; fi
  SRV=""
}

ok() { CHECKS=$((CHECKS + 1)); printf 'ok   — %s\n' "$1"; }
fail() { printf 'FAIL — %s\n' "$1" >&2; exit 1; }
expect() { [ "$2" = "$3" ] && ok "$1: $2" || fail "$1: expected $3, got $2"; }

json() { python3 -c '
import sys, json
d = json.load(sys.stdin)
for k in sys.argv[1:]:
    d = d[int(k)] if isinstance(d, list) else d[k]
print(d if not isinstance(d, (dict, list)) else json.dumps(d))
' "$@"; }

rpc() { # rpc <base> <token> <request>
  local token=$2
  curl -s -m 8 -X POST "http://$1/mcp" -H "Authorization: Bearer $token" \
    -H 'Content-Type: application/json' -d "$3"
}

http_code() {
  local base=$1 token=${2:-} auth=()
  if [ -n "$token" ]; then auth=(-H "Authorization: Bearer $token"); fi
  curl -s -o /dev/null -w '%{http_code}' -m 8 -X POST "http://$base/mcp" \
    "${auth[@]}" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"ping"}'
}

mint() { # mint <state-dir> -> one-time code
  XDG_DATA_HOME="$1" "$BIN" mcp pair-code | sed -n 's/.*pairing code: \([0-9]\{6\}\).*/\1/p'
}

pair() { # pair <base> <name> <code> -> token
  curl -s -m 8 -X POST "http://$1/pair" -H 'Content-Type: application/json' \
    -d "{\"name\":\"$2\",\"code\":\"$3\"}" | json token
}

echo "== 1. a LAN listener is refused until an Agent is paired =="
EMPTY="$WORK/unpaired"; mkdir -p "$EMPTY"; chmod 700 "$EMPTY"
if start "$EMPTY" "${LAN_IP:-192.0.2.1}:$PORT" true; then
  stop; fail "service started on a non-loopback address with no paired Agent"
fi
grep -q "Refusing a non-loopback MCP listener" "$EMPTY/serve.log" \
  && ok "boot blocked with: $(tail -1 "$EMPTY/serve.log")" \
  || fail "unexpected refusal: $(tail -3 "$EMPTY/serve.log")"

echo "== 2. loopback transport, pairing and authorisation =="
LB="$WORK/loopback"; mkdir -p "$LB"; chmod 700 "$LB"
start "$LB" "127.0.0.1:$PORT" || fail "service did not come up: $(cat "$LB/serve.log")"
expect "health over loopback" "$(curl -s -m 3 "http://127.0.0.1:$PORT/health" | json status)" "ok"
expect "MCP without a token" "$(http_code "127.0.0.1:$PORT" "")" "401"
expect "MCP with a forged token" "$(http_code "127.0.0.1:$PORT" "vsh_not_a_real_credential")" "401"

CODE=$(mint "$LB"); [ "${#CODE}" = "6" ] || fail "no pairing code was minted"
TOKEN=$(pair "127.0.0.1:$PORT" "smoke-loopback" "$CODE")
[ "${#TOKEN}" = "47" ] || fail "pairing returned no token: ${TOKEN:0:6}…"
ok "one-time code exchanged for a 47-character Agent token"
expect "the same code replayed" \
  "$(curl -s -o /dev/null -w '%{http_code}' -m 8 -X POST "http://127.0.0.1:$PORT/pair" \
      -H 'Content-Type: application/json' -d "{\"name\":\"replay\",\"code\":\"$CODE\"}")" "401"

EP=127.0.0.1:$PORT
expect "initialize" "$(rpc "$EP" "$TOKEN" '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}' | json result serverInfo name)" "zxyssh"
expect "notifications/initialized body" \
  "$(curl -s -o /dev/null -w '%{http_code}' -m 8 -X POST "http://$EP/mcp" -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","method":"notifications/initialized"}')" "202"
TOOLS=$(rpc "$EP" "$TOKEN" '{"jsonrpc":"2.0","id":2,"method":"tools/list"}')
COUNT=$(printf '%s' "$TOOLS" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)["result"]["tools"]))')
[ "$COUNT" = "7" ] && ok "tools/list advertises the 7-tool Agent surface" || fail "tools/list returned $COUNT tools"
expect "tools/call server_list" \
  "$(rpc "$EP" "$TOKEN" '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"server_list","arguments":{}}}' | json result content 0 text)" "[]"
expect "unknown method" \
  "$(rpc "$EP" "$TOKEN" '{"jsonrpc":"2.0","id":4,"method":"bogus/method"}' | json error code)" "-32601"
python3 -c 'import json; print(json.dumps({"jsonrpc":"2.0","id":5,"method":"ping","params":{"blob":"A" * 18_000_000}}))' > "$WORK/big.json"
expect "18 MiB request hits the body limit" \
  "$(curl -s -o /dev/null -w '%{http_code}' -m 15 -X POST "http://$EP/mcp" -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' --data-binary "@$WORK/big.json")" "413"
rm -f "$WORK/big.json"
grep -q -e "$CODE" -e "$TOKEN" "$LB/serve.log" && fail "the service log contains a pairing code or token" \
  || ok "service log contains neither the pairing code nor the token"

echo "== 3. audit rows are written encrypted at rest =="
DB=$(find "$LB" -name zxyssh.db | head -1); [ -n "$DB" ] || fail "no database was created in the state directory"
python3 - "$DB" <<'PY' || exit 1
import sqlite3, sys
rows, enc = sqlite3.connect(sys.argv[1]).execute(
    "select count(*), coalesce(sum(payload like 'enc:v1:%'), 0) from agent_activity").fetchone()
if rows == 0 or rows != enc:
    sys.exit(f"audit rows={rows} encrypted={enc}")
print(f"ok   — {rows} audit rows, all encrypted with the device key")
PY

echo "== 4. the same listener on the LAN interface, then instant revocation =="
stop
LAN="$LB"
if [ -z "$LAN_IP" ]; then
  echo "skip — no global IPv4 address found, cannot exercise a LAN bind"
elif ! start "$LAN" "$LAN_IP:$PORT" true; then
  # A runner may report a scope-global address it cannot bind (TEST-NET, or an
  # address added by a network namespace that rejects assignment). That is an
  # environment limit, not a gateway regression: section 1 already proved the
  # guard refuses non-loopback binds without a paired Agent, and sections 2-3
  # prove transport, pairing, limits and audit over loopback.
  if grep -q 'Cannot assign requested address' "$LAN/serve.log"; then
    echo "skip — $LAN_IP is not bindable in this environment"
  else
    fail "LAN bind refused: $(tail -2 "$LAN/serve.log")"
  fi
else
  ok "listener opened on $LAN_IP:$PORT because an Agent was already paired"
  LEP=$LAN_IP:$PORT
  LCODE=$(mint "$LAN")
  LTOKEN=$(pair "$LEP" "smoke-lan" "$LCODE")
  [ "${#LTOKEN}" = "47" ] || fail "LAN pairing returned no token"
  ok "an Agent paired over $LAN_IP"
  expect "tools/list over the LAN address" \
    "$(curl -s -m 8 -X POST "http://$LEP/mcp" -H "Authorization: Bearer $LTOKEN" -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":6,"method":"tools/list"}' | python3 -c 'import sys,json; print(len(json.load(sys.stdin)["result"]["tools"]) == 7)')" "True"
  AGENT=$(XDG_DATA_HOME="$LAN" "$BIN" mcp agents | awk '/smoke-lan/{print $1}')
  XDG_DATA_HOME="$LAN" "$BIN" mcp revoke "${AGENT:0:8}" >/dev/null
  expect "the revoked token on its very next request" "$(http_code "$LEP" "$LTOKEN")" "401"
  ok "revocation took effect without restarting the service"
fi

stop
echo "all $CHECKS checks passed"
