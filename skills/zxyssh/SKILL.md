---
name: zxyssh
description: Use the zxySSH SSH service through its MCP endpoint for site listing, persistent terminal sessions, and server-managed sudo; pair an Agent with a one-time code that an operator must approve, and administer the service with the native CLI.
---

# zxySSH

Connect to the zxySSH MCP endpoint over HTTP. Do not use Node.js, `gateway.mjs`, browser automation, raw `ssh`/`scp`, or credential extraction.

The service runs on a Linux host and owns the SSH connections. Credentials, private keys, passphrases, and sudo passwords stay on that host: an Agent sees terminal input and output, nothing else.

Resolve the native executable in this order: `zxyssh` from `PATH`, then `$HOME/.local/bin/zxyssh`. The executable administers the service; it no longer opens SSH sessions itself. If neither path exists, tell the user which lookup failed instead of substituting another client, because that bypasses the saved site configuration and the session model.

## Pair an Agent

An operator mints a single-use six-digit code, either in the desktop app (Settings → **Agent access and pairing**) or on the host:

```bash
zxyssh mcp pair-code
```

Pairing then waits for a human. Exchanging the code creates a request, and no credential exists until an operator approves it:

```text
POST /pair {"name":"<agent name>","code":"<six digits>","clientPublicKey":"<base64url X25519>"}
     -> 202 {"requestId":"...","status":"awaiting_approval","pollPath":"/pair/status"}
POST /pair/status {"requestId":"..."}
     -> 200 {"status":"awaiting_approval"}
     -> 200 {"status":"delivered","agentId":"...","name":"...","serverPublicKey":"...","sealedToken":"...","tokenType":"Bearer-sealed"}
```

Approve or refuse from either side, on the same registry:

```bash
zxyssh mcp requests list
zxyssh mcp requests approve <requestId|prefix>
zxyssh mcp requests reject <requestId|prefix>
zxyssh mcp approvals            # show the mode
zxyssh mcp approvals --set off  # issue credentials immediately again
```

With the approval queue on, `clientPublicKey` is required: a keyless client could not read a sealed delivery, so `POST /pair` answers `400` and leaves the code unspent. Turn the queue off (`zxyssh mcp approvals --set off`) to restore the one-step shape `POST /pair {"name","code"}` -> `200 {"token":...}`, which returns the credential in the clear. With a key, the credential is sealed through an ephemeral X25519 agreement so a listener on the segment cannot read it, and it is delivered once. `zxyssh mcp pair --endpoint <url> --name <name>` is the reference client: it reads the code from stdin, waits for approval, and prints the credential once.

The code expires after 300 seconds, five wrong guesses spend it, and an unanswered request expires after 900 seconds. Keep tokens out of prompts, logs, and files. `zxyssh mcp agents` lists paired Agents; `revoke`, `rename`, and `rotate` change credentials, and a revoked or rotated token stops working on the next request.

The listener defaults to `127.0.0.1:8765`. Reaching it over a LAN requires `ZXYSSH_MCP_BIND=<address>:<port>` with `ZXYSSH_MCP_ALLOW_LAN=true`, a concrete address, and at least one already-paired Agent; the service publishes what it bound to in `$XDG_DATA_HOME/zxyssh/mcp-lan-status.json`, which the desktop shows. This transport is plain HTTP for a trusted internal network: approval and sealing protect the credential, while the pairing code itself still crosses the wire in the clear.

## Use the MCP tools

JSON-RPC 2.0 over `POST /mcp` with `Authorization: Bearer <token>`. `initialize`, `notifications/initialized`, `ping`, `tools/list`, and `tools/call` are supported; `/health` stays unauthenticated.

Paired Agents get exactly these tools:

| Tool | Arguments | Result |
| --- | --- | --- |
| `server_list` | none | Saved sites as `{id, name}` pairs |
| `session_create` | `server_name` (required) | A new session the Agent owns |
| `session_send_input` | `session_id`, `data`, optional `keys`, `append_enter` | Bytes delivered to that PTY |
| `session_sudo` | `session_id`, `command` | Output of an allowlisted privileged command |
| `session_read` | `session_id`, optional `max_bytes` (up to 65536) | Trailing terminal output |
| `session_resize` | `session_id`, `cols`, `rows` | Updated PTY size |
| `session_kill` | `session_id` | That session terminated |

Call `tools/list` instead of reciting this table when a live schema is needed; the schema is authoritative. Any other tool name, including arbitrary `exec`, remote `rg`, SFTP transfer, and site CRUD, is not offered to a paired Agent and is rejected when requested.

## Workflow

1. `server_list`, then choose a site by the name the person configured. Never guess a host, port, or username.
2. `session_create` with `server_name`. Each paired Agent receives its own new session; reusing another Agent's session is refused.
3. `session_send_input` to type, `session_read` to observe. Read again rather than assuming a command finished: input being accepted only proves bytes were sent.
4. Answer an interactive prompt with literal text or a named key from `keys` (`enter`, `tab`, `escape`, `backspace`, `ctrl-c`, `ctrl-d`, `ctrl-z`, `up`, `down`, `left`, `right`). Use `ctrl-c` to interrupt instead of opening a second connection.
5. `session_kill` when finished. Batch termination is not available to an Agent.

Host-key trust is checked by the service before authentication, including the real destination behind a jump host. An unexpected or changed fingerprint fails the connection; do not retry it into an approval. If a site needs trust established without a desktop, an operator runs `zxyssh mcp hostkey probe <server>` and then trusts the shown key.

## Privileged commands

`session_sudo` runs a command as root only when a person has configured both a sudo credential and an exact allowlist for that site. The service decrypts the password and injects it over the SSH channel; the Agent never receives or supplies it.

- Request the command exactly as configured. Near matches, shell metacharacter tricks, and unlisted commands are refused.
- Missing credential, missing policy, or an interactive password change are refused rather than escalated.
- Never ask for the sudo password, and never put any secret in `data`. Passwords and keys are not commands to log.

## Human and Agent collaboration

The desktop shows Agent operations in its activity history, and an Agent session appears as its own tab without stealing the person's active tab. Shared interactive input and independent inspection are both visible; a person's terminal remains theirs.

Before anything destructive, read the current state first, then state what will change. Overwrites, recursive transfers, and service restarts need the person's explicit approval. Do not claim success from a submission alone; report the site, session, exit status, and relevant output.

## Plugin references

`references/` documents the built-in plugin actions that ship with the repository. They describe remote inspection capabilities, and they grant nothing: no permission, no installation, no elevation, no confirmation. These actions are not exposed to a paired Agent in this version, so do not guess an action name or parameter in order to reach them.

| Built-in plugin | Reference |
| --- | --- |
| Server performance | [server-performance](references/server-performance.md) |
| Docker containers | [docker-containers](references/docker-containers.md) |
| Kubernetes pods | [kubernetes-pods](references/kubernetes-pods.md) |
| Database inspector | [database-inspector](references/database-inspector.md) |
| Redis inspector | [redis-inspector](references/redis-inspector.md) |
| Process explorer | [process-explorer](references/process-explorer.md) |
| System logs | [system-logs](references/system-logs.md) |
| Network inspector | [network-inspector](references/network-inspector.md) |
| Disk usage | [disk-usage](references/disk-usage.md) |
| Git workspace | [git-workspace](references/git-workspace.md) |
| Cron scheduler | [cron-scheduler](references/cron-scheduler.md) |
| Systemd services | [systemd-services](references/systemd-services.md) |

## Operational rules

- Inspect before mutating. Read the current file, service, or directory state before editing, deleting, restarting, or overwriting.
- Keep secrets on the host. Do not request private-key text or passwords; saved site profiles already provide authentication.
- One session per Agent per task. Prefer continuing an owned session over repeatedly creating new ones.
- Treat an owned session as live work. Another person may be looking at it, so do not send input that assumes they are away.
- A connection lives only while its owning process lives. Upgrade the desktop and the service together, and save ongoing work before restarting either.
