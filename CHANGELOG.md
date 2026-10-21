# Changelog

## 2.0.1 — 2026-10-22

### zxySSH, with an Agent-facing MCP service at the centre

The application is rebranded as **zxySSH**. The package, binaries, Skill directories, systemd units, environment variables, release metadata and documentation move to the `zxyssh` name, and the bundle identifier becomes `com.zxyssh.desktop`.

**Automatic migration from a pre-rebrand install was removed.** The 2.0.1 build reads only its own data directory, so an existing installation keeps its saved servers, encrypted credentials, host fingerprints and activity history only if `vibeshell.db`, `ssh_fingerprints.json`, `ssh_credentials.key` and `ssh_credentials.salt` are copied from `~/.local/share/com.vibeshell.desktop` to `~/.local/share/com.zxyssh.desktop` before the first start, keeping the `ssh_credentials.*` names and renaming the database to `zxyssh.db`. Without that step the app starts with an empty site list, and a *newly generated* device key cannot decrypt the old ciphertext, so back up the previous directory first. The Skill installer still recognises the `vibeshell` and `vshell` directories in order to delete them.

The product now has one job: run SSH on a Linux host and expose it to AI Agents through MCP. `zxyssh mcp serve` answers JSON-RPC over `/mcp` and publishes `/health` and `/pair`. Agents pair with a single-use six-digit code, receive a bearer token, and are administered entirely from the service CLI (`pair-code`, `agents`, `revoke`, `rename`, `rotate`, `hostkey`, `credential`). A LAN listener requires an explicit bind address plus `ZXYSSH_MCP_ALLOW_LAN=true`, refuses wildcard addresses, and refuses to start while no Agent is paired.

### Credentials stay on the host

Paired Agents see only terminal input and output. The server-side connect path uses stored credentials directly, so saved SSH passwords, private keys and passphrases no longer cross into the frontend or an Agent prompt. Agent tool discovery is limited to `server_list`, `session_create`, `session_send_input`, `session_sudo`, `session_read`, `session_resize` and `session_kill`; `exec`, `rg`, SFTP transfers, remote file editing and server CRUD remain unavailable over the paired HTTP transport, and a request for a withheld tool is rejected.

Agents create sessions by saved site name only, always get a fresh session, and can subsequently touch only sessions they created. `session_kill` no longer accepts the batch form for a paired Agent: an Agent can terminate its own single session and nothing else.

Sudo is delegated without exposing the password. Each site can store a sudo credential and an exact allowlist of non-interactive commands; the service decrypts the password, injects it over the SSH channel and refuses anything not configured, anything needing an interactive password prompt, and anything where no policy exists.

### Desktop as the site manager

The interface follows the MobaXterm shape a person expects: a site tree with nested groups and connection status on the left, tabbed terminals in the centre, SFTP and the file workspace alongside, and an activity panel that shows what an Agent did. Adding or editing a site also captures the sudo password and its command allowlist.

Everything unrelated to this workflow is removed rather than hidden: the plugin marketplace, plugin panels and plugin workspace, command history, snippets, quick commands, the tunnel panel, workspace-change views, the coding-agent launcher and its dedicated terminal mode.

### Linux-only delivery and operator guides

Release packaging builds Linux x64 desktop and native CLI artifacts only; macOS and Windows distributions are not produced or signed. `scripts/test-lan-mcp-gateway.sh` drives a real listener through pairing, bearer authentication, body limits, audit-at-rest and revocation inside a throwaway state directory, so the gateway can be verified with one command without touching operator data. `docs/LAN-MCP-UBUNTU-DEBIAN.md` is the operator runbook, including listener guard rules, and a same-user systemd user unit shares the desktop database with the service.

### Compatibility notes

This is a major version: the CLI no longer provides `ssh`, `sftp`, `servers`, `sessions`, `daemon`, `import`, `plugins`, `tools`, `install` or `uninstall` commands, so scripts and Agent prompts that called them must move to the MCP tools. GUI and service must be upgraded together; a connection only lives as long as its owning process. Vague or partial SSH behaviour is still not promised, and third-party stored passwords are never copied during profile import.

## 1.1.0 — 2026-09-18

### A shared workspace for people and agents

Agent and native CLI commands now have visible operation notifications and durable, encrypted local history. Repeated and multiline commands retain separate records. Agent-created SSH sessions appear as distinct UI tabs without taking focus from a person's active tab.

GUI-first and daemon-first workflows route terminal, SFTP, tunnel, recording and database-probe requests to the process that owns the session. Native plugin discovery, action schemas, live reference documents and guarded execution are available through both CLI and MCP. All 12 built-in plugin references are generated from the same validated manifests used at runtime.

### Safer editing and transfers

Saved passwords, private keys and passphrases can be edited without recreating a server. Omitted fields preserve stored values; metadata and secret changes commit atomically. Credential loading no longer replaces existing key material on corruption or read failures, and first-time key initialization is serialized.

SFTP downloads wait for local file writes before reporting completion. Directory sync detects same-size content changes, protects excluded paths and nested ignore rules from deletion, and rejects overlapping local roots. PTYs can reopen after a remote shell exits; forwarding includes readiness, cancellation and half-close handling.

### UX and packaging

Searchable connection launchers, keyboard focus handling, reduced-motion support, split/detached file workspaces and explicit error states keep terminal work in context. The connection-card animation now uses browser-native APIs; GSAP and its React wrapper have been removed.

The desktop, native CLI, workspace crates, Tauri configuration and agent marketplace metadata share version 1.1.0. English, Simplified Chinese and Japanese READMEs explain workflows and limits rather than claiming universal SSH compatibility.

### Project governance and licensing

The project as a whole moves to **GPL-3.0-only**. Earlier MIT permissions and attribution remain preserved in `licenses/legacy-MIT.txt`; third-party licenses remain their own. Release packaging includes license notices and matching source materials.

`dev` is the default integration branch; `main` is the stable release branch. Feature and fix PRs target `dev`, followed by release-promotion PRs from `dev` to `main`. Publication is tag-driven and stays a draft until validation and all release jobs succeed.

### Not included / compatibility notes

The proposed CLI server-create/delete and Teleport integrations are not merged into 1.1.0; they require changes to preserve credential and file-operation guarantees. Mobile parity, every MFA/hardware-token flow, and every remote plugin dependency are not promised. Remote performance collection assumes Linux `/proc`.

An owner process must remain alive to retain its SSH connections. Upgrade the desktop and CLI together and save ongoing work before restarting either. Existing dependency advisories are not resolved merely by passing functional tests.
