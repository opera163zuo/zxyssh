# Changelog

## Unreleased

### Pairing follows the listener address

`zxyssh mcp pair` no longer assumes its service is on loopback. When the
operator omits `--endpoint`, it uses the live listener address already
published in zxySSH's local service status file; an explicit
`--endpoint` still takes precedence, and loopback remains the fallback when no
service has published a live status. This lets a local Agent pair with a
listener bound to a specific LAN interface without copying its IP by hand.

### The pairing panel says what it means now

The countdown and the Agent count rendered as `%{seconds}` and `%{agents}`: those
two strings were the only ones in the catalog written in Rust's interpolation
syntax, while i18next is configured with its default `{{ }}`, so nothing was ever
substituted. They interpolate now, and the third `%{}` string left behind by the
deleted approval queue is gone with it.

A code you have shown somebody is a code you may need to replace, so the digits
are now the button: click them and the panel mints a fresh code, which
invalidates the one on screen. And a pairing that succeeds is now visible the
second it succeeds. The panel used to poll the Agent list on a five-second timer
that could never fire — its effect re-ran every second because it listed the
ticking clock among its dependencies — so a spent code just sat there counting
down to nothing. It now asks the registry directly, through a new
`mcp_pair_code_pending`, every two seconds, and the first answer of "no" clears
the code, reloads the list and names the Agent that arrived.

### Settings stopped offering to install what it already installs

The **Skills** section — the per-tool cards for installing the zxySSH Skill into
Claude Code, Codex and friends — is deleted, along with its three Tauri commands
(`detect_ai_tools`, `install_to_tool`, `uninstall_from_tool`), `IntegrationCard`,
the store fields behind it and the installer functions only it called. The desktop already writes the Skill into its default tool set at startup, which
is what makes the Skill reachable to an Agent in the first place; the section
only ever re-ran that by hand.

## 2.2.0 — 2026-11-01

### Pairing is the code, nothing else

The approval queue is gone, not defaulted off. In 2.1.0 a correct six-digit code
only opened a request and nothing was issued until an operator approved it, which
made an unattended Agent look dead whenever nobody was sitting at the desktop to
click. Now `POST /pair {"name","code"}` returns the credential in one step, as in
2.0.1, and the operator surface that decided requests has been deleted with it:
`zxyssh mcp requests`, `zxyssh mcp approvals`, the five desktop commands behind
them, and the approval checkbox and waiting list in Settings -> Agent access and
pairing.

Credential sealing stays. A client that sends `clientPublicKey` still gets its
token through an ephemeral X25519 agreement fetched from `POST /pair/status`, so
a listener on the same segment cannot read it, and it is still handed out
exactly once. That handshake answers `202` with the status name
`awaiting_approval` — kept so Agents written against 2.1.0 keep parsing it — but
the state now means "sealed, poll for it"; no human is in the loop. A keyless
client receives its credential in the clear, as before 2.1.0.


### The desktop stopped pretending to be Windows

The native menu bar is built explicitly now, so Linux shows **Edit** and
**Workspace** only. The empty **Window** submenu and the duplicate Help/About
dialog came from Tauri's default menu, which the app no longer uses.

Settings lost two surfaces that never did anything: **Window Opacity** (labelled
"Windows only", and no backend ever read the value) and the whole **SSH
Defaults** section — default port, connection timeout, keepalive interval and
default username were written to the settings blob and read by nobody, while
the site editor keeps its own port and the SSH client keeps its own timeouts.
**Session recording → storage location** now shows the directory the recorder
actually writes to instead of a hard-coded `%APPDATA%` path.

Saved settings keep working: the loader ignores the two dropped keys and stops
writing them on the next change.

### Code with no way in was deleted

Four surfaces had lost their entry point but were still compiled, shipped and
documented:

- **Port forwarding.** `src-tauri/src/tunnel/` and its runtime requests are
  gone. No MCP tool, CLI command or Tauri command could start a tunnel, so the
  manager only ever stopped tunnels that never existed. Jump hosts are
  unaffected — they bridge through a `direct-tcpip` channel inside
  `SessionManager` and never used this code. A server that now asks to forward
  a channel back to us is rejected outright instead of being matched against an
  always-empty registry. The `tunnel_configs` table stays, as do the other
  legacy tables, so an existing database is still readable.
- **Database console.** The plugin dashboards (`src/components/PluginDashboards/`),
  `dbConnectionsStore`, `commands/dbconn.rs` and `src-tauri/src/dbconn/` are
  deleted, which also drops the `tokio-postgres`, `mysql_async` and
  `tokio-stream` dependencies. The built-in plugin catalog and its generated
  reference documents are unchanged; only the unreachable UI went.
- **Config importers.** `src-tauri/src/ssh_import/` (OpenSSH, PuTTY, Tabby) had
  no command registration and no caller.
- **Dead capability flags.** `backgroundTunnels` and `cliIpc` were reported by
  the backend and read by nobody but the tests that asserted they existed.

The frontend tunnel store and its types went with them.

### Four surfaces we no longer ship

Settings lost **AI Command Prediction** (BYOK ghost text), **Session Recording**,
**Cloud Sync** (Gist / WebDAV / portable file) and every **mobile** code path.

The mobile branch was the easy one: `src-tauri/gen/` holds no Android or iOS
project, CI builds Linux only, and `tauri build` cannot produce a mobile
artifact — so `local_shell_mobile.rs`, `capabilities/mobile.json`, the mobile
icons, `MobileWorkspaceActions`, `RuntimePlatform::{Ios, Android}` and the
`isMobile` branches were all unreachable.

Prediction went with its settings blob and API-key field; the local
history/path completion and its ghost-text overlay stay. Recording lost the
`SessionLogger`, its six Tauri commands and the tab context-menu items; the
`recordings` table and its storage layer remain, as the other legacy tables do.
Cloud Sync lost the vault manager, its providers, its commands, the pairing-code
UI and the autosave hook in `serverStore`.

Deleting the recording commands also removed the last producer of the
`SessionService` IPC request, so that variant and its dispatcher are gone too:
the service socket now carries session I/O only.

The sync engine went with it. `src-tauri/src/storage/sync.rs` (~4.3k lines) and
`sync_crypto.rs` (~390) are deleted, which removes the outbox, the revision and
tombstone bookkeeping, the conflict convergence rules, the vault cursors and the
batch envelope format. Every server, group, snippet and plugin save had been
writing a JSON change into `sync_outbox` that nothing had read since the Cloud
Sync UI was removed; those sixteen writes are gone, and `server_delete` /
`group_delete` now run their own detach updates — clearing `group_id`,
`parent_id` and `jump_host_id`, columns that carry no foreign key — instead of
calling into the sync module to do it. `Database::cloud_sync()` no longer exists,
so the four tables (`sync_metadata`, `sync_entity_state`, `sync_outbox`,
`sync_vault_state`) are no longer created at all.

Nothing about the on-disk data of an existing installation breaks: SQLite does
not drop the old tables, and leaving them is harmless because no code reads them.
The `recordings.sync_status` column stays in the schema for the same reason, but
the `Recording` model no longer carries a field for it, and its three readers no
longer select it — which also means a database written before that column
existed can be listed again.

Renaming an Agent-facing surface is not the same as testing it, so the storage
invariants that the sync suite was carrying by accident now have their own file:
`src-tauri/tests/database_domain_test.rs` runs against the real `init_schema()`
and asserts that a deleted server cascades its declared foreign keys, keeps
recording rows, detaches its dependents' `jump_host_id`, and that a deleted group
detaches both child groups and member sites. The other 30 tests in `sync.rs`
tested the outbox and died with it.

### Windows stopped being a target we pretend to support

The app never shipped a Windows build, but it carried the Windows half of every
platform decision: an NSIS/WiX installer block pointing at a `nsis-hooks.nsh`
that is not in this repository, a `.wxs` PATH fragment, `.ico` and
`Square*Logo` assets, a `windows-sys` and a `winreg` dependency, named-pipe
transport in the service IPC, a `GetAsyncKeyState` docking gesture, PowerShell
and `cmd` in the local-shell detector, and `wmic` in the status panel. All of it
is gone, and `ipc/socket.rs` now reads as the Unix-socket implementation it is.

`bundle.targets` is `["deb", "appimage"]` instead of "whatever the host can
build", `capabilities/default.json` lists `linux` and `macOS`, and
`scripts/check-release.mjs` plus `src-tauri/tests/acl_and_icons_config_test.rs`
now assert that instead of the old triple. `RuntimePlatform` no longer has a
`Windows` variant, so the settings screen and title bar dropped their
Windows-only branches too.

## 2.1.0 — 2026-10-22

### Pairing became an access decision, not a handoff

A correct six-digit pairing code no longer buys a credential. It opens a **pairing request** carrying the requested Agent name and the source address, and nothing is issued until an operator decides. The desktop app now manages that queue in Settings → **Agent access and pairing**: mint a code with its countdown, approve or refuse each request, rename, rotate and revoke paired Agents, and see what the LAN listener actually bound to. The same registry is driven from a headless host with `zxyssh mcp requests list|approve|reject` and `zxyssh mcp approvals [--set on|off]`, because both views read and write the one encrypted registry in the shared database.

Approved credentials are sealed rather than displayed. The Agent may send an ephemeral `clientPublicKey`; the service then performs an X25519 agreement, derives an AES-256-GCM key scoped to that request id, and returns the sealed token only through `POST /pair/status` — once, after which the request keeps no credential material. A listener on the same network segment can no longer recover a bearer token from a pairing exchange. While the queue is on, a pairing request without `clientPublicKey` is refused with `400` and the code stays unspent; `zxyssh mcp approvals --set off` restores the one-step cleartext exchange.

`zxyssh mcp pair --endpoint <url> --name <name>` is the reference client: it reads the code from stdin so it stays out of arguments and shell history, waits for the approval, and prints the credential once. Requests go unanswered after 900 seconds, the spent code cannot open a second request, and a registry written by 2.0.1 is read as *requiring approval* rather than silently keeping one-step pairing.

### Compatibility notes

Pairing is two-step by default for new and existing installations. An operator who wants 2.0 behaviour runs `zxyssh mcp approvals --set off` or unticks the setting; that is also the only mode in which a client sending no `clientPublicKey` can pair in one request. `POST /pair` answers `202` with a `requestId` instead of a token while approval is pending, so any script that parsed a token straight out of the pairing response must poll `/pair/status`. `/health`, `/pair` and `/pair/status` remain unauthenticated; every MCP route still requires a bearer token. The pairing code itself still travels in the clear over this plain-HTTP transport — approval and sealing protect the credential, not the code.


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
