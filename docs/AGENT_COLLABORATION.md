# Human–Agent collaboration in zxySSH 2.0

[Overview](../README.md) · [简体中文](../README.zh-CN.md) · [Plugin specification](plugin-spec.md)

## One host owns the credentials

The desktop and the MCP service read the same saved sites. Credentials, private keys, passphrases and sudo passwords are decrypted only inside the Rust service. They do not cross into the React frontend, an Agent prompt, an activity record or a log line.

A paired Agent is a terminal user, not an operator of the host. It can list sites by name, open its own session, type into that session, read its output, resize it and close it. Arbitrary `exec`, remote search, SFTP transfer, uploads, local files and site CRUD are not on the paired-Agent tool surface; asking for them is rejected rather than silently degraded.

## Visible operations without interfering with a person

The workspace activity panel shows each Agent operation with its tool, session, timestamp and lifecycle state. Repeated calls stay separate records and multiline input is retained in full; older entries page in. The UI combines event notification with incremental polling while active, and recovers history after a restart. Polling is not a promise of zero latency.

`started` means an operation was accepted for processing; `succeeded` means that API operation completed; `failed` includes errors and denied operations. For terminal input, success means the bytes were delivered, not that the remote command succeeded.

Agent-created sessions appear as their own tabs and never steal the person's active tab. Two connections to the same server stay two sessions distinguished by session identity.

The activity store is locally encrypted and is not cloud-synced. SFTP events record operation and path context, not file contents. Saved credentials and protected input are never logged as commands, and an ordinary command string can itself contain a secret, so do not put secrets in arguments.

A transport with no approval UI attached fails a confirmation-requiring call closed instead of running it unattended. Approval means a person saw this command, on this session, before it ran.

## Sessions and process ownership

Terminal input, resize, read, sudo and kill are routed to the process that owns the selected SSH session. The GUI owns the service when it starts one; a same-user service started by systemd can own sessions that outlive a GUI window, while GUI-owned connections end when that process exits.

If a reply is lost after input was sent, the client does not replay it. A transport error cannot prove the operation never ran, so retrying a mutation needs a real idempotency argument rather than an optimistic assumption.

## Privileged commands through the service

Each site can store a sudo password and an exact list of non-interactive commands. `session_sudo` requires both: the service decrypts the password, injects it over the SSH channel, and refuses anything not configured, anything needing an interactive password change, and anything where no policy exists. The Agent never supplies or receives the password, so pressure to "just pass the sudo password" has nowhere to go.

## Editing saved credentials

The server add/edit dialog holds the SSH credential, the sudo password and the sudo command allowlist. Untouched fields preserve stored values and existing secrets are not loaded back into the form, so an edit cannot echo a stored secret. Metadata, renames and secret updates share one SQLite transaction; failure rolls back the whole edit, and a name conflict does not delete another server's credentials. Closing the dialog clears secret drafts and invalidates pending key-file reads, preventing a stale async result from overwriting a newer choice.

This changes zxySSH's saved login information, not the remote OS account password. A valid private key also does not establish the server's identity: host-key verification remains necessary, including the real destination behind a jump host.

## Plugin runtime in this version

`plugins/builtin/<id>/plugin.json`, `BUILTIN_MANIFESTS` and `docs/plugin-spec.md` remain in the repository, and the reference catalog under `skills/zxyssh/references/` is still parity-checked by `scripts/check-release.mjs`. Since the 2.0 narrowing, no plugin action is reachable by a person or an Agent: the marketplace and panel UI were removed, the CLI plugin commands were removed, and plugin tools are not advertised over MCP.

Treat the reference documents as maintenance-time descriptions of a runtime that is currently not exposed. Do not implement an Agent-facing plugin call by assuming a `plugins` command or a `plugin_*` MCP tool exists; adding that surface is a tool-allowlist, permission and approval change that needs its own design and tests.

Regenerate repository references with `cargo run --locked -p zxyssh-plugins --example export_references`; add `-- --check` in CI. This touches no user directories or live sessions.

## Validation boundaries

Regression tests cover the Agent tool allowlist, session ownership, batch-kill refusal, pairing, revocation, body limits, audit-at-rest, atomic credential edits and reference/catalog parity. `scripts/test-lan-mcp-gateway.sh` drives a real listener end to end inside a throwaway state directory. The loopback OpenSSH fixture covers common authentication, PTY and SFTP behavior.

These checks do not prove every external server, MFA flow or platform works. Plain HTTP carries the bearer token in clear text, so it is only acceptable on a trusted internal network; UI interaction, binary packaging, signature validation and installed-app behavior need separate verification. Keep desktop, service and Skill versions aligned during upgrades, and preserve active work before restarting an owner process.
