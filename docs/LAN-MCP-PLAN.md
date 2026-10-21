# zxySSH LAN MCP — project plan

## Goal

Build a Linux-hosted zxySSH MCP service, targeting Ubuntu and Debian servers, so Hermes Agents on different operating systems can connect over the local network and operate SSH sessions managed by that zxySSH instance.

The Linux server owns SSH connections, saved server profiles, host-key verification, SFTP, and the MCP endpoint. Remote Agents use the standard MCP HTTP transport and do not need to run zxySSH or SSH locally.

## Current status

Branch `feature/lan-mcp-gateway` in the `opera163zuo/zxyssh` fork; draft PR: https://github.com/opera163zuo/zxyssh/pull/1

Phases 0-4 are implemented and verified end-to-end on a Debian 13 host against a local sshd fixture:

- The scaffold that did not compile (`include!("pairing_http.rs")` inside `server.rs`) is fixed; pairing HTTP is a real module now, and the desktop gateway is back to loopback-only.
- `zxyssh mcp serve` runs headless: Streamable-HTTP JSON-RPC on `/mcp` (`initialize`, `notifications/initialized` answered with `202`, `ping`, `tools/list`, `tools/call`), `/health`, `/pair`.
- Pairing codes are one-time, 5-minute, 5-attempt, constant-time compared, per-peer rate limited, and persisted so an administrator can mint a code from a different process than the running service.
- Agent bearer tokens are issued once, stored only as SHA-256 hashes in the device-encrypted registry, and reloaded from disk before every request, so revoke and rotate take effect immediately.
- Risky commands fail closed on this transport because there is no approval UI; every audited action records the paired Agent identity.
- `zxyssh mcp credential set|clear` and `zxyssh mcp hostkey probe|trust|list` let a headless server actually reach a target machine without a GUI, with secrets accepted only on stdin and host keys pinned only after the operator repeats the fingerprint.
- systemd unit, backup/restore and firewall/TLS procedures are documented in `docs/LAN-MCP-UBUNTU-DEBIAN.md`.
- `bash scripts/test-lan-mcp-gateway.sh [address]` re-runs the whole gate (18 checks: bind guard, pairing, bearer auth, `202` notifications, body limit, encrypted audit, instant revocation) against a throwaway state directory, so an operator can verify a deployment without touching saved user data.

Not yet delivered, and required before this is called production-ready:

- TLS terminates outside the process only (reverse proxy or documented cleartext-VLAN guidance); there is no in-process HTTPS.
- No `Origin`/browser-origin rejection and no per-Agent session or server allowlists — paired Agents share one database and session manager, so they are not isolated tenants.
- Linux/Ubuntu service lifecycle was exercised locally; Phase 5 cross-OS Hermes verification (macOS, Windows, and a second Linux Agent) has not been run.

## Functional requirements

1. Run headlessly on supported Ubuntu/Debian server releases, with no GUI, display server, or Node.js runtime required after installation.
2. Provide a standard Streamable HTTP MCP endpoint addressable by Hermes Agents on macOS, Windows, Linux, and other systems with HTTP MCP support.
3. Pair Agents through an explicit, short-lived one-time code. Return a cryptographically random independent token only after successful pairing.
4. Store only token hashes; encrypt persistent pairing records at rest; never put tokens in URLs or logs.
5. Let an administrator list paired Agents and revoke or rotate each Agent credential independently.
6. Bind to loopback by default. LAN binding must be explicit, require at least one valid Agent credential, and support a specific private interface address rather than silently exposing all interfaces.
7. Preserve audit records and existing command-approval checks. If a risky operation needs human approval and no approval UI exists, fail closed.
8. Support a dedicated system user, protected persistent state, systemd startup/restart, graceful shutdown, and documented backup/restore.

## Security requirements

- Use bearer authorization headers, constant-time token-hash comparison, request/body/output limits, and rate limiting for pairing attempts.
- Enforce HTTPS/TLS in the service or require a documented trusted TLS proxy for LAN traffic; reject accidental cleartext exposure where feasible.
- Protect the initial pairing code: show it only to the local administrator on the server console, expire it quickly, limit attempts, and permit one successful use.
- Require host-key verification for SSH and preserve current credential secrecy boundaries.
- Scope each Agent's access to explicit permissions and, where practical, server/session allowlists. Initial version may start with a clearly documented common permission profile, but must not imply identities are isolated if they share the same database/session manager.
- Keep secrets out of process arguments, environment dumps, logs, and Git.
- Systemd service must run unprivileged with a private state directory and least-privilege filesystem/network settings. Do not ship a unit whose hardening blocks the configured state directory.

## Implementation phases

Status: Phase 0-4 are implemented and locally verified (see *Current status*). Phase 5 is outstanding.

### Phase 0 — Stabilize the branch — done

- Build and test the currently pushed commit from a clean checkout.
- Fix malformed wiring or revert unsafe/partial route changes.
- Keep an explicit incomplete status until all acceptance tests pass.

### Phase 1 — MCP transport — done

- Wire `mcp-serve` to a real Axum HTTP server using the existing MCP JSON-RPC handler.
- Support `initialize`, `notifications/initialized`, `ping`, `tools/list`, and `tools/call` over the supported MCP HTTP transport.
- Add integration tests that start the listener on an ephemeral loopback port and exercise those methods.

### Phase 2 — Pairing and authentication — done

- Add a separate pairing endpoint, unauthenticated only while presenting a valid one-time code.
- Make pairing atomically consume the code and issue a token once.
- Protect every MCP endpoint with per-Agent bearer token authentication; leave only health and pairing endpoints public.
- Test valid token, missing token, invalid token, revoked token, expired code, repeated code, attempt-limit, and persisted restart behavior.
- Never print a bearer token. Display the pairing code locally; redact it from logs after expiry.

### Phase 3 — Linux service lifecycle — partial

- Add configuration for bind address, port, state/database path, logs, and TLS/proxy mode.
- Add a systemd unit with a dedicated user, explicit writable state path, restart policy, restrictive umask, and safe shutdown.
- Document install, configuration, firewall, TLS, pairing, health-check, upgrade, and rollback on Ubuntu/Debian.
- Verify service lifecycle on real Ubuntu and Debian environments (or CI VMs), not just macOS builds. **Outstanding: only Debian 13 was exercised here; a real Ubuntu host or CI VM is still needed.**

### Phase 4 — Agent management and audit — done

- Add admin-only local CLI commands to list/revoke/rename paired Agents and rotate credentials.
- Ensure encrypted at-rest records survive restarts and backup/restore, and fail closed on decryption/corruption instead of silently resetting the registry.
- Record Agent identity with each MCP action in the audit trail.

### Phase 5 — Cross-platform Hermes verification — outstanding

- Configure a Hermes Agent on Linux, macOS, and Windows to use the same remote MCP URL with separate Agent tokens.
- Verify `tools/list`, harmless SSH inspection, SFTP read, and an explicitly approved mutating command.
- Confirm revoked credentials stop working immediately and one Agent's token cannot be used to impersonate another identity.

## Acceptance criteria

- A clean Ubuntu/Debian host can install and run the service as an unprivileged systemd user.
- A local admin obtains a short-lived pairing code; a remote Hermes Agent pairs from another OS and receives one token exactly once.
- That Agent successfully initializes MCP, lists tools, and performs an authorized test operation through the LAN endpoint.
- Unauthenticated, malformed, expired, over-limit, or revoked requests are rejected; LAN mode never starts without the intended guardrails.
- Pairing records persist encrypted across service restart; corrupt/unreadable records cause an explicit safe failure.
- Logs and command history contain no pairing code or bearer token.
- Integration tests and platform build/test matrix pass; Ubuntu/Debian install and service run are verified before calling the feature complete.

## Out of scope for this phase

- Removing macOS or Windows desktop releases from upstream zxySSH.
- Public Internet exposure or multi-tenant hosting.
- Replacing SSH authentication or bypassing human approval for risky operations.
- Claiming production readiness based solely on compilation or local unit tests.
