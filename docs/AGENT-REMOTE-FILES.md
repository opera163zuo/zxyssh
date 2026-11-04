# Remote file access for AI Agents over MCP

zxySSH 2.5.0 put the first SFTP tools on the paired-Agent surface, and the
release after it adds byte transfer, so an Agent can now place and fetch binary
files as well as text. This document describes how an Agent uses them, what the
service enforces, and which limits are consequences of the design rather than
bugs to work around.

Everything here was checked against the running service, and `bash
scripts/test-mcp-binary-transfer.sh` repeats those checks in CI: a paired Agent, a
live session on a loopback OpenSSH container, a 1 MiB non-UTF-8 file moved out and
read back in pages, and each refusal below observed on the wire. The authoritative
schema is `tools/list`; read it instead of copying the tables below into a
prompt.

## What an Agent gets

A paired Agent holds a bearer token and can call exactly twelve tools. Seven
drive terminals; five touch files on the **target host**:

| Tool | Purpose | Content over the MCP wire? |
| --- | --- | --- |
| `sftp_ls` | List a directory on the target host | No — a listing comes back |
| `sftp_read` | Read a remote **text** file | Yes, inbound |
| `sftp_write` | Write a remote **text** file | Yes, outbound |
| `sftp_get_bytes` | Read any **byte range** of a remote file as base64 | Yes, inbound |
| `sftp_put_bytes` | Write **any bytes** to a remote file, SHA-256 verified | Yes, outbound |

The last two exist because everything here moves **content**, never a path on the
machine running zxySSH. No Agent-facing tool accepts a local path.

The transport is MCP Streamable HTTP: `POST /mcp`, JSON-RPC 2.0, with
`Authorization: Bearer <token>`.

## Prerequisite: a session you own

All five file tools take a `session_id`. That is not a connection handle — it is
an SSH session the service already owns, created from a saved site profile.

```text
POST /mcp   {"jsonrpc":"2.0","id":1,"method":"tools/call",
             "params":{"name":"server_list","arguments":{}}}
     -> [{"id":"…","name":"prod-web"}]

POST /mcp   {"jsonrpc":"2.0","id":2,"method":"tools/call",
             "params":{"name":"session_create","arguments":{"server_name":"prod-web"}}}
     -> {"id":"<session_id>", …}
```

`session_create` for a paired Agent requires `server_name`; `server_id` is not
offered, and the service always forces a new session rather than handing back an
existing one.

**Ownership is enforced before any SFTP opens.** The service refuses a
`session_id` that the calling Agent did not create, with the JSON-RPC error
`Agent is not authorized to access this session`. This is a permission check,
not a hint: it applies identically to typing and to file access, so one Agent
cannot read another Agent's target through the file tools.

## `sftp_ls`

```json
{ "session_id": "…", "path": "/etc/ssh", "show_hidden": false }
```

`path` defaults to the remote home directory; `show_hidden` defaults to `false`
and dotfiles are skipped. Relative paths resolve against that home directory,
not against whatever `cd` a terminal happens to be sitting in.

The result is plain text, directories first, then names case-insensitively:

```text
Directory: /etc/ssh

-rw-r--r--   1.6K ssh_config
-rw-------    548 ssh_host_ed25519_key
drwxr-xr-x      - sshd_config.d/

Total: 3 entries
```

Sizes are human-formatted, so they are for reading, not for comparison. If you
need an exact byte count, `sftp_read` reports the truncation ratio, or run
`wc -c` through `session_send_input`.

## `sftp_read`

```json
{ "session_id": "…", "path": "/etc/ssh/sshd_config", "max_bytes": 1048576 }
```

- `max_bytes` defaults to **1 MiB**.
- A file larger than `max_bytes` comes back prefixed with
  `[Truncated: showing N/M bytes]`. Raising `max_bytes` is the operator's call,
  not something to loop over. The cut is moved back to a character boundary, so
  the reported `N` can be a few bytes below your cap.
- A file that is **not valid UTF-8 fails at any size** with
  `File '…' contains non-UTF-8 binary content.` `render_remote_text` validates
  the whole payload before truncating, so this tool can never return a binary
  file as lossy text that you then write back corrupted. For bytes that are not
  text, use `sftp_get_bytes`.

Read-only. It never writes, and there is no offset parameter — you cannot page
through a large log with it. Use `session_send_input` with `sed -n`/`tail` for
that.

## `sftp_write`

```json
{ "session_id": "…", "path": "/etc/profile.d/env.sh", "content": "export FOO=bar\n" }
```

Returns `Written <N> bytes to '<resolved path>'`.

Three behaviours worth knowing before relying on them:

1. **It overwrites.** The file is opened with `CREATE | TRUNCATE`. There is no
   `overwrite: false` guard on this tool and no backup. `sftp_read` first if the
   file might matter.
2. **Missing parent directories are created for you.** `sftp_write` goes through
   `write_remote_file`, which uses `WriteRemoteFileOptions::default()` — and that
   default is `create_parent_dirs: true, overwrite: true`
   (`src-tauri/src/sftp/helpers.rs`). So writing `/opt/app/conf/x.yaml` performs a
   recursive `MKDIR` of `/opt/app/conf` if the first `OPEN` fails. This is a side
   effect, not a convenience: do not point the tool at a path whose parent you
   have not checked, because it will materialise directories on the target.
   (`sftp_write` has no `parents` argument — unlike `add_file`, which exposes it.
   The default is not something an Agent can turn off.)
3. **`content` is a JSON string, so it must be UTF-8.** Shell heredoc quoting
   problems disappear, but binary content cannot be expressed here — use
   `sftp_put_bytes` for bytes.

## `sftp_put_bytes`

```json
{ "session_id": "…", "path": "/opt/app/agent.tgz",
  "data_base64": "H4sIAAAAAAAAA…", "sha256": "3f2a1b…", "mode": "overwrite" }
```

Returns `Wrote 1048576 bytes to '/opt/app/agent.tgz' (sha256 3f2a1b…, mode
overwrite, verified by read-back)`.

`sha256` is **required**, and it must describe the bytes you actually sent: the
service hashes the decoded payload and refuses the call before writing anything
if it disagrees. `mode` is `overwrite` (default) or `create_new`, which refuses a
path that already exists.

What the service guarantees, in order:

1. The payload decodes as standard base64 and is at most **8 MiB** after decoding.
2. Its SHA-256 equals the `sha256` you declared. Nothing is written if not.
3. The parent directory of `path` already exists. Unlike `sftp_write`, this tool
   **does not create directories** — placing an artifact should not mint trees on
   the target as a side effect.
4. The bytes go to a sibling temporary file (`<path>.zxyssh-part-<id>`), which is
   then **read back from the target and hashed again**. Only a match renames the
   temporary file onto `path`.
5. On any mismatch the temporary file is removed and `path` keeps whatever it had
   before. A failed transfer therefore cannot leave a truncated file where the
   next command would pick it up. The error says whether the temporary file was
   removed or left behind, so nothing is hidden from the operator.

Because plain `SSH_FXP_RENAME` does not overwrite on every server, replacing an
existing file may take a remove-then-rename step; that window is the one part of
this that is not atomic.

One consequence worth knowing: the content is never written *through* `path`, it
is moved onto it. If `path` is a symlink, the link is replaced by a regular file
rather than followed, which is not what `sftp_write` does — that tool opens
`path` directly, so a symlink there is written through.

## `sftp_get_bytes`

```json
{ "session_id": "…", "path": "/var/log/app.db", "offset": 0, "length": 1048576 }
```

Returns a JSON document as text:

```json
{ "path": "/var/log/app.db", "offset": 0, "length": 1048576,
  "total_bytes": 7340032, "eof": false, "encoding": "base64",
  "sha256": "6c1e…", "data_base64": "…" }
```

`length` defaults to 1 MiB and is capped at **4 MiB** per call; page with
`offset`, and stop when `eof` is true. `sha256` covers the returned range, not the
whole file, so it proves what you received rather than what the file is. An
`offset` past the end of the file is an error, and a directory is refused rather
than opened.

The `path` is resolved on the target host and reported back in full — read the
result to confirm where it actually landed.

## Worked example

Deploy a small drop-in config and verify it:

```text
1. session_create  {"server_name":"prod-web"}
2. sftp_ls         {"session_id":"S","path":"/etc/ssh/sshd_config.d"}
3. sftp_read       {"session_id":"S","path":"/etc/ssh/sshd_config.d/99-hardening.conf"}
      -> error, file does not exist yet        (expected; nothing was overwritten)
4. sftp_write      {"session_id":"S","path":"/etc/ssh/sshd_config.d/99-hardening.conf",
                    "content":"PermitRootLogin no\nMaxAuthTries 3\n"}
      -> Written 40 bytes to '/etc/ssh/sshd_config.d/99-hardening.conf'
5. session_send_input  "sudo sshd -t"           (or session_sudo if allowlisted)
```

Step 5 matters: writing a config file is not the same as the service accepting
it. Confirm before reporting success.

## What stays closed, and why

`sftp_upload`, `sftp_download`, `sftp_upload_directory` and
`sftp_sync_directory` exist in the codebase but are **refused for paired
Agents** (`-32601`, "Tool '…' is not available to paired Agents").

They take a `local_path` that the **service host** reads from its own disk — the
Agent's own filesystem is never involved, because nothing about the Agent's
machine crosses the wire. Handing those tools to a remote Agent would mean any
holder of a token could point `local_path` at `~/.ssh/id_rsa` or the app-data
directory and have the contents written out to whatever target host it chooses.
That is an exfiltration path, not a file transfer.

So the rule is simple: **content travels in the request body, or it does not
travel at all.** Text uses `sftp_read` / `sftp_write`, bytes use `sftp_get_bytes` /
`sftp_put_bytes`, and nothing on the Agent surface can name a path on the service
host. What is still a human action — or an out-of-band transfer the operator
performs deliberately — is anything bigger than one call: multi-hundred-megabyte
artifacts and whole directory trees.

`exec`, remote `rg` and site CRUD are refused for the same kind of reason: they
are not part of what a terminal user needs.

## Limits and failure modes

| Situation | What happens |
| --- | --- |
| Request larger than 16 MiB | HTTP `413` before any tool runs |
| `session_id` belongs to another Agent | `-32602` "Agent is not authorized to access this session" |
| Site unreachable / session dead | Tool error naming the session; host-key mismatch fails before authentication |
| Remote path is a directory | `Remote upload path is not a file: …` |
| Permission denied on target | SFTP error passes through; sudo is not implied |
| Non-UTF-8 file via `sftp_read` | Error at any size; 2.5.0 returned lossy text when the file exceeded `max_bytes` |
| `sha256` does not match the payload | Error before any write; nothing lands on the target |
| Target hashes differently after the write | Temporary file removed, `path` unchanged |
| Parent directory missing via `sftp_put_bytes` | Error naming the parent; nothing created |
| `mode=create_new` onto an existing path | Error, existing file untouched |
| More than 8 MiB out (`sftp_put_bytes`) / 4 MiB back (`sftp_get_bytes`) | Error before the transfer; a request body over 16 MiB is `413` |

Plain HTTP carries the bearer token in clear text. This is acceptable only on a
trusted internal network with the operator's explicit opt-in
(`ZXYSSH_MCP_ALLOW_LAN=true`, a specific interface address, at least one paired
Agent). Do not expose the endpoint to untrusted networks.

Every permitted call is recorded in the desktop's encrypted activity history
with the Agent's name and id, the session, and the lifecycle state. An Agent
cannot hide what it did from the operator.
