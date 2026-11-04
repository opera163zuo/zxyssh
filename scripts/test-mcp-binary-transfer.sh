#!/bin/bash
# End-to-end binary transfer check for the paired-Agent surface.
#
# A paired Agent moves real bytes onto an isolated, loopback-only OpenSSH
# container through a live `zxyssh mcp serve` listener, and reads them back over
# the same transport. Unlike scripts/test-lan-mcp-gateway.sh, which never opens a
# target connection, this exercises the whole path: HTTP request, bearer auth,
# session ownership, base64 payload, SHA-256 verification and the rename onto the
# target path.
#
# Every state file lives in a throwaway directory under target/, and the site,
# credential and host key are created there only — the operator's servers,
# credentials and paired Agents are never read or written.
#
#   usage: bash scripts/test-mcp-binary-transfer.sh
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=target/debug/zxyssh
mkdir -p target
WORK=$(mktemp -d "$PWD/target/mcp-binary-transfer.XXXXXX")
chmod 700 "$WORK"
STATE="$WORK/state"
SSH_DIR="$WORK/ssh"
mkdir -p "$STATE" "$SSH_DIR"
chmod 700 "$STATE" "$SSH_DIR"
SRV=""
CONTAINER=""
CHECKS=0
EP=""
TOKEN=""

cleanup() {
  if [ -n "$SRV" ] && kill -0 "$SRV" 2>/dev/null; then kill -TERM "$SRV" 2>/dev/null || true; fi
  if [ -n "$CONTAINER" ]; then docker rm -f "$CONTAINER" >/dev/null 2>&1 || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

for tool in curl docker python3 sha256sum ssh-keygen; do
  command -v "$tool" >/dev/null || { echo "missing dependency: $tool" >&2; exit 1; }
done
[ -x "$BIN" ] || cargo build -p zxyssh --bin zxyssh

ok() { CHECKS=$((CHECKS + 1)); printf 'ok   — %s\n' "$1"; }
fail() { printf 'FAIL — %s\n' "$1" >&2; exit 1; }
expect() { [ "$2" = "$3" ] && ok "$1: $2" || fail "$1: expected $3, got $2"; }
contains() {
  case "$2" in
    *"$3"*) ok "$1" ;;
    *) fail "$1: expected to find '$3' in '$2'" ;;
  esac
}
starts_with_zero() { case "$1" in 0*) return 0 ;; *) return 1 ;; esac; }

json() { python3 -c '
import sys, json
d = json.load(sys.stdin)
for k in sys.argv[1:]:
    d = d[int(k)] if isinstance(d, list) else d[k]
print(d if not isinstance(d, (dict, list)) else json.dumps(d))
' "$@"; }

rpc() { # rpc <token> <request> -> raw JSON-RPC response
  # The body goes through a file: a 1 MiB payload arrives as a megabyte-scale
  # argument, and curl would be refused by the kernel before it connected.
  local body="$WORK/request.json"
  printf '%s' "$2" > "$body"
  curl -s -m 60 -X POST "http://$EP/mcp" -H "Authorization: Bearer $1" \
    -H 'Content-Type: application/json' --data-binary "@$body"
}

call() { # call <token> <id> <tool> <arguments-json> -> result text
  rpc "$1" "{\"jsonrpc\":\"2.0\",\"id\":$2,\"method\":\"tools/call\",\"params\":{\"name\":\"$3\",\"arguments\":$4}}" \
    | json result content 0 text
}

# Several tools return a JSON document as their text content; this reaches into
# that document instead of treating the text itself as the value.
result_value() { # result_value <key> reads stdin, prints one field of that document
  python3 -c '
import json, sys

body = json.load(sys.stdin)
print(json.loads(body["result"]["content"][0]["text"])[sys.argv[1]])
' "$1"
}

error_code() { # error_code <token> <id> <tool> <arguments-json> -> JSON-RPC error code
  rpc "$1" "{\"jsonrpc\":\"2.0\",\"id\":$2,\"method\":\"tools/call\",\"params\":{\"name\":\"$3\",\"arguments\":$4}}" \
    | json error code
}

error_message() { # error_message <token> <id> <tool> <arguments-json> -> JSON-RPC error message
  rpc "$1" "{\"jsonrpc\":\"2.0\",\"id\":$2,\"method\":\"tools/call\",\"params\":{\"name\":\"$3\",\"arguments\":$4}}" \
    | json error message
}

in_container() { docker exec "$CONTAINER" sh -c "$1"; }

# Arguments for a small upload whose digest honestly describes its content.
put_args() { # put_args <session-id> <path> <content> [mode]
  python3 -c '
import base64, hashlib, json, sys

arguments = {
    "session_id": sys.argv[1],
    "path": sys.argv[2],
    "data_base64": base64.b64encode(sys.argv[3].encode()).decode(),
    "sha256": hashlib.sha256(sys.argv[3].encode()).hexdigest(),
}
if len(sys.argv) > 4 and sys.argv[4]:
    arguments["mode"] = sys.argv[4]
print(json.dumps(arguments))' "$1" "$2" "$3" "${4:-}"
}

# The fixture's own digest, read inside the container: the point is that the
# bytes on the target match, not that our client thinks they do.
target_sha() { in_container "sha256sum $1" | awk '{print $1}'; }
absent() { [ "$(in_container "ls $1 2>/dev/null | wc -l")" = "0" ]; }

echo "== 1. an isolated OpenSSH fixture, reachable on loopback only =="
ssh-keygen -q -t ed25519 -N '' -f "$SSH_DIR/ed25519"
cp "$SSH_DIR/ed25519.pub" "$SSH_DIR/authorized_keys"
docker build -q --tag zxyssh-ponytail-sshd --file scripts/ssh-fixture/Dockerfile scripts/ssh-fixture >/dev/null
CONTAINER=$(docker run --detach --publish 127.0.0.1::22 zxyssh-ponytail-sshd)
export SSH_PORT
SSH_PORT=$(docker port "$CONTAINER" 22/tcp | awk -F: '{print $NF}')
docker cp "$SSH_DIR/authorized_keys" "$CONTAINER:/home/audit/.ssh/authorized_keys"
in_container 'chown audit:audit /home/audit/.ssh/authorized_keys; chmod 600 /home/audit/.ssh/authorized_keys'
docker exec "$CONTAINER" cat /etc/ssh/ssh_host_ed25519_key.pub > "$SSH_DIR/host.pub"
HOST_FP=$(ssh-keygen -lf "$SSH_DIR/host.pub" | awk '{print $2}')
case "$HOST_FP" in SHA256:*) ;; *) fail "could not read the fixture host key fingerprint" ;; esac
ok "sshd is up on 127.0.0.1:$SSH_PORT"

echo "== 2. a site, its key credential and a trusted host key in a throwaway profile =="
# Any CLI command opens the database through the path the service uses, which is
# how the schema exists before the site row is inserted.
XDG_DATA_HOME="$STATE" HOME="$STATE" "$BIN" mcp agents >/dev/null 2>&1 || true
DB=$(find "$STATE" -name zxyssh.db | head -1)
[ -n "$DB" ] || fail "no database was created in the state directory"
python3 - "$DB" <<'PY'
import sqlite3, sys, time, uuid

with sqlite3.connect(sys.argv[1]) as database:
    database.execute(
        """INSERT INTO servers (id, name, host, port, username, auth_type, tags, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
        (
            str(uuid.uuid4()),
            "binary-fixture",
            "127.0.0.1",
            int(__import__("os").environ["SSH_PORT"]),
            "audit",
            "key",
            "[]",
            int(time.time()),
            int(time.time()),
        ),
    )
PY
ok "the site row exists before any credential does"
XDG_DATA_HOME="$STATE" HOME="$STATE" "$BIN" mcp credential set \
  binary-fixture --auth-type key --key-path "$SSH_DIR/ed25519" </dev/null >/dev/null
ok "a key credential is stored for it"
XDG_DATA_HOME="$STATE" HOME="$STATE" "$BIN" mcp hostkey trust 127.0.0.1 \
  --port "$SSH_PORT" --fingerprint "$HOST_FP" >/dev/null
ok "the fixture host key is trusted, decided by the operator and not the Agent"

echo "== 3. a paired Agent opens a live session on that site =="
PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')
EP=127.0.0.1:$PORT
XDG_DATA_HOME="$STATE" ZXYSSH_MCP_BIND="$EP" "$BIN" mcp serve > "$STATE/serve.log" 2>&1 &
SRV=$!
for _ in $(seq 1 40); do
  curl -fs -m 1 "http://$EP/health" >/dev/null 2>&1 && break
  kill -0 "$SRV" 2>/dev/null || fail "service exited: $(tail -3 "$STATE/serve.log")"
  sleep 0.25
done
expect "health over loopback" "$(curl -s -m 3 "http://$EP/health" | json status)" "ok"

mint() { XDG_DATA_HOME="$STATE" "$BIN" mcp pair-code | sed -n 's/.*pairing code: \([0-9]\{6\}\).*/\1/p'; }
pair() { # pair <name> -> token, spending the code through the shipped client
  local code
  code=$(mint)
  [ "${#code}" = "6" ] || fail "no pairing code was minted"
  printf '%s\n' "$code" | XDG_DATA_HOME="$STATE" HOME="$STATE" "$BIN" mcp pair \
    --name "$1" > "$STATE/pair-$1.log" 2>&1 \
    || fail "pairing client failed: $(cat "$STATE/pair-$1.log")"
  sed -n 's/.*sealed: \(vsh_[A-Za-z0-9_-]*\).*/\1/p' "$STATE/pair-$1.log"
}

TOKEN=$(pair binary-smoke)
[ "${#TOKEN}" = "47" ] || fail "pairing returned no token"
ok "an Agent paired and holds its own credential"

contains "server_list shows the site by name" "$(call "$TOKEN" 1 server_list '{}')" "binary-fixture"
CREATED=$(rpc "$TOKEN" '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"session_create","arguments":{"server_name":"binary-fixture"}}}')
expect "the Agent's session connected to the fixture" "$(printf '%s' "$CREATED" | result_value state)" "connected"
SESSION=$(printf '%s' "$CREATED" | result_value id)
[ -n "$SESSION" ] || fail "session_create returned no session id: $(printf '%s' "$CREATED" | head -c 300)"
ok "a live SSH session belongs to this Agent"

echo "== 4. bytes go out: the write is verified on the target =="
# `agent_activity` stores one encrypted blob per event, so the trail is checked by
# how much it grew rather than by reading columns that are not there.
audit_rows() {
  python3 -c '
import sqlite3, sys

print(sqlite3.connect(sys.argv[1]).execute("SELECT count(*) FROM agent_activity").fetchone()[0])
' "$DB"
}
BEFORE=$(audit_rows)
BLOB="$WORK/blob.bin"
python3 -c 'open(__import__("sys").argv[1],"wb").write(bytes(i % 256 for i in range(1_048_576)))' "$BLOB"
BLOB_SHA=$(sha256sum "$BLOB" | awk '{print $1}')
ARGS=$(python3 -c '
import base64, hashlib, json, sys

blob = open(sys.argv[1], "rb").read()
print(json.dumps({
    "session_id": sys.argv[2],
    "path": "/home/audit/blob.bin",
    "data_base64": base64.b64encode(blob).decode(),
    "sha256": hashlib.sha256(blob).hexdigest(),
}))' "$BLOB" "$SESSION")
contains "the byte tool reports a verified write" "$(call "$TOKEN" 3 sftp_put_bytes "$ARGS")" "verified by read-back"
expect "the target holds exactly the bytes sent" "$(target_sha /home/audit/blob.bin)" "$BLOB_SHA"
expect "the target reports the size we sent" "$(in_container 'stat -c %s /home/audit/blob.bin')" "1048576"
expect "no temporary file survived the rename" "$(in_container 'ls /home/audit | grep -c zxyssh-part || true')" "0"

echo "== 5. bytes come back: ranged read over the same transport =="
PAGE_ARGS='"offset":700000,"length":700000'
ONE=$(rpc "$TOKEN" "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"sftp_get_bytes\",\"arguments\":{\"session_id\":\"$SESSION\",\"path\":\"/home/audit/blob.bin\",\"offset\":0,\"length\":700000}}}")
TWO=$(rpc "$TOKEN" "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"sftp_get_bytes\",\"arguments\":{\"session_id\":\"$SESSION\",\"path\":\"/home/audit/blob.bin\",$PAGE_ARGS}}}")
expect "the first page says more remains" "$(printf '%s' "$ONE" | result_value eof)" "False"
expect "the first page knows the whole size" "$(printf '%s' "$ONE" | result_value total_bytes)" "1048576"
expect "the second page reaches the end" "$(printf '%s' "$TWO" | result_value eof)" "True"
# Both pages are megabyte-scale, so they go through files rather than argv.
printf '%s' "$ONE" > "$WORK/page-one.json"
printf '%s' "$TWO" > "$WORK/page-two.json"
python3 - "$WORK/page-one.json" "$WORK/page-two.json" "$BLOB" <<'PY' || fail "the paged read did not reassemble the file"
import base64, json, sys

def body(path):
    with open(path) as handle:
        return json.loads(json.load(handle)["result"]["content"][0]["text"])

one, two = body(sys.argv[1]), body(sys.argv[2])
reassembled = base64.b64decode(one["data_base64"]) + base64.b64decode(two["data_base64"])
with open(sys.argv[3], "rb") as handle:
    original = handle.read()
assert one["length"] == 700000 and two["length"] == len(original) - 700000, (one, two)
assert reassembled == original, "paged bytes differ from the file"
PY
ok "two base64 pages reassemble to the original file, byte for byte"
contains "sftp_read still refuses what it cannot carry" \
  "$(call "$TOKEN" 6 sftp_read "{\"session_id\":\"$SESSION\",\"path\":\"/home/audit/blob.bin\"}")" \
  "non-UTF-8 binary content"

echo "== 6. the refusals are the point of the design =="
contains "a digest that does not describe the payload is refused" \
  "$(call "$TOKEN" 7 sftp_put_bytes "$(python3 -c '
import base64, json, sys

print(json.dumps({
    "session_id": sys.argv[1],
    "path": "/home/audit/blob.bin",
    "data_base64": base64.b64encode(b"pretend this is the payload").decode(),
    "sha256": "0" * 64,
}))' "$SESSION")")" \
  "does not describe the payload"
expect "the refusal left the real file alone" "$(target_sha /home/audit/blob.bin)" "$BLOB_SHA"

contains "create_new refuses an existing path" \
  "$(call "$TOKEN" 8 sftp_put_bytes "$(python3 -c '
import base64, hashlib, json, sys

payload = b"content whose digest is honestly declared"
print(json.dumps({
    "session_id": sys.argv[1],
    "path": "/home/audit/blob.bin",
    "data_base64": base64.b64encode(payload).decode(),
    "sha256": hashlib.sha256(payload).hexdigest(),
    "mode": "create_new",
}))' "$SESSION")")" \
  "already exists"
expect "and again the real file is untouched" "$(target_sha /home/audit/blob.bin)" "$BLOB_SHA"

contains "a missing parent directory is not created" \
  "$(call "$TOKEN" 9 sftp_put_bytes "$(python3 -c '
import base64, hashlib, json, sys

payload = b"x"
print(json.dumps({
    "session_id": sys.argv[1],
    "path": "/home/audit/absent/deep/blob.bin",
    "data_base64": base64.b64encode(payload).decode(),
    "sha256": hashlib.sha256(payload).hexdigest(),
}))' "$SESSION")")" \
  "Parent directory"
absent "/home/audit/absent" && ok "nothing was created on the way" || fail "a directory tree appeared"

contains "a directory is refused as a target" \
  "$(call "$TOKEN" 10 sftp_put_bytes "$(put_args "$SESSION" /home/audit "hi")")" \
  "is a directory"

contains "a payload over the per-call cap is refused" \
  "$(call "$TOKEN" 11 sftp_put_bytes "$(python3 -c '
import base64, hashlib, json, sys

payload = bytes(i % 251 for i in range(9 * 1024 * 1024))
print(json.dumps({
    "session_id": sys.argv[1],
    "path": "/home/audit/too-big.bin",
    "data_base64": base64.b64encode(payload).decode(),
    "sha256": hashlib.sha256(payload).hexdigest(),
}))' "$SESSION")")" \
  "too large"
absent "/home/audit/too-big.bin" && ok "nothing landed for the refused upload" || fail "an oversized file was written"

expect "the host-local upload tool stays refused" \
  "$(error_code "$TOKEN" 12 sftp_upload "{\"session_id\":\"$SESSION\",\"local_path\":\"/etc/passwd\",\"remote_path\":\"/home/audit/stolen\"}")" "-32601"
absent "/home/audit/stolen" && ok "and it did not reach the target either" || fail "host-local transfer leaked through"

echo "== 7. ownership is enforced before the payload is touched =="
SECOND=$(pair binary-other)
[ "${#SECOND}" = "47" ] || fail "the second Agent did not pair"
# The second Agent's own session, so the later calls below are a control for the
# refusals above rather than a second mistake.
OTHER=$(rpc "$SECOND" '{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"session_create","arguments":{"server_name":"binary-fixture"}}}' | result_value id)
[ -n "$OTHER" ] || fail "the second Agent could not open its own session"
expect "a second Agent cannot read through the first Agent's session" \
  "$(error_message "$SECOND" 14 sftp_get_bytes "{\"session_id\":\"$SESSION\",\"path\":\"/home/audit/blob.bin\"}")" \
  "Agent is not authorized to access this session"
expect "and cannot write through it either" \
  "$(error_code "$SECOND" 15 sftp_put_bytes "$(put_args "$SESSION" /home/audit/hijack.bin "hi")")" \
  "-32602"
absent "/home/audit/hijack.bin" && ok "no file appeared from the refused attempt" || fail "an unauthorized write landed"
contains "the second Agent still owns its own session" \
  "$(call "$SECOND" 16 sftp_ls "{\"session_id\":\"$OTHER\",\"path\":\"/home/audit\"}")" "blob.bin"
contains "its own write goes through" \
  "$(call "$SECOND" 17 sftp_put_bytes "$(python3 -c '
import base64, hashlib, json, sys

payload = b"written by the second Agent"
print(json.dumps({
    "session_id": sys.argv[1],
    "path": "/home/audit/second.bin",
    "data_base64": base64.b64encode(payload).decode(),
    "sha256": hashlib.sha256(payload).hexdigest(),
}))' "$OTHER")")" \
  "Wrote 27 bytes to '/home/audit/second.bin'"

echo "== 8. the audit trail records the operation, never the content =="
AFTER=$(audit_rows)
# Each permitted call writes a start row and a terminal row, both encrypted with
# the device key. So: the trail grew, nothing sits in the clear, and neither the
# file bytes nor their base64 appear in any stored file.
python3 - "$DB" "$BLOB" "$BEFORE" "$AFTER" <<'PY' || fail "the audit trail is missing or leaked content"
import base64, glob, sqlite3, sys

path, blob_path, before, after = sys.argv[1:5]
rows, encrypted = sqlite3.connect(path).execute(
    "SELECT count(*), sum(payload LIKE 'enc:v1:%') FROM agent_activity"
).fetchone()
assert rows == int(after) and encrypted == rows, f"unencrypted audit rows: {encrypted}/{rows}"
assert int(after) - int(before) >= 10, f"the byte transfers were not recorded: {before} -> {after}"

raw = open(blob_path, "rb").read()
needle_raw = raw[700_000:700_064]
needle_text = base64.b64encode(raw[700_000:700_064]).decode()
stored = b"".join(open(name, "rb").read() for name in glob.glob(path + "*"))
assert needle_raw not in stored, "file bytes were stored in the activity database"
assert needle_text.encode() not in stored, "base64 content was stored in the activity database"
print(f"{int(after) - int(before)} audit rows added by the transfers, all encrypted, no content in storage")
PY
ok "the transfers are audited without their content reaching storage"

# Revocation is decided by Agent id, not by the bearer credential the Agent holds.
SECOND_ID=$(XDG_DATA_HOME="$STATE" HOME="$STATE" "$BIN" mcp agents | awk '/binary-other/{print $1}')
[ -n "$SECOND_ID" ] || fail "the second Agent is not listed"
XDG_DATA_HOME="$STATE" HOME="$STATE" "$BIN" mcp revoke "${SECOND_ID:0:8}" >/dev/null
expect "a revoked token is refused on its next request" \
  "$(curl -s -o /dev/null -w '%{http_code}' -m 8 -X POST "http://$EP/mcp" \
     -H "Authorization: Bearer $SECOND" -H 'Content-Type: application/json' \
     -d '{"jsonrpc":"2.0","id":18,"method":"ping"}')" "401"
contains "and the first Agent is still working" \
  "$(call "$TOKEN" 19 sftp_ls "{\"session_id\":\"$SESSION\",\"path\":\"/home/audit\"}")" "second.bin"

echo
printf 'all %d checks passed\n' "$CHECKS"
