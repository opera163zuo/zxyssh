# zxySSH — Agent Guide

## Overview
zxySSH is a modern SSH/SFTP desktop terminal built with **Tauri 2** (Rust backend) and **React 18** (TypeScript frontend). It manages SSH sites, multi-session terminals, SFTP file operations, local shell, session recording, jump hosts, and an MCP service that lets a paired AI Agent drive those terminals without ever holding a credential.

## Branch and release workflow

All feature, fix and documentation PRs target `dev`, the default integration branch. `main` accepts release promotions from the same repository's `dev`; See CONTRIBUTING.md. CI compilation and release packaging/publication are **manual-only** (`workflow_dispatch`); neither pushes, PRs nor tags trigger them. PR Target remains automatic and all existing required CI checks remain enforced: a maintainer runs CI on the PR head before merging. Release runs from `main`, requires successful manual CI on the exact tagged commit, and defaults to an unpublished draft unless `publish=true` is explicitly selected. Do not auto-bump versions or dispatch publication without authorization. Never move or recreate an existing tag. This repository is standalone (not a fork) and its history starts at 2.0.1, so the first tag is `v2.0.1` on a `main` commit that has a successful manual CI run. zxySSH has been GPL-3.0-only since 1.1.0; the upstream attribution in NOTICE and `licenses/legacy-MIT.txt` must be preserved even though the git history begins here.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Frontend (React 18 + Vite 6 + TypeScript)              │
│  ├── Zustand stores                                    │
│  ├── xterm.js terminal emulator                         │
│  └── Tailwind CSS (Tokyo Night theme)                   │
├─────────────────────────────────────────────────────────┤
│  IPC: Tauri invoke (frontend↔backend)                   │
│  IPC: Named pipe / Unix socket (CLI↔backend)            │
├─────────────────────────────────────────────────────────┤
│  Backend (Rust / Tauri 2)                               │
│  ├── SSH: russh 0.63 (async, shared session ownership)  │
│  ├── SFTP: russh-sftp                                   │
│  ├── DB: rusqlite (SQLite, bundled)                     │
│  ├── Async: tokio runtime                               │
│  └── Local Shell: portable-pty                          │
└─────────────────────────────────────────────────────────┘
```

## Critical Knowledge

### SSH ownership constraints
- `client::Handle<H>` and `Channel<S>` are **NOT Clone**
- Handle sharing: wrap in `Arc<tokio::sync::Mutex<Option<Handle>>>`
- Channel I/O: use `channel.into_stream()` → `tokio::io::split()` for bidirectional data

### Frontend → Backend Communication
- All Tauri calls go through `src/lib/tauri.ts` → `safeInvoke<T>(command, args)`
- Returns `InvokeResult<T>` = `{ success: true; data: T }` | `{ success: false; error: TauriError }`
- For latency-sensitive input: use `sendInputBatched()` with RAF batching
- For fire-and-forget: use `fireAndForgetInvoke()`

### Session Lifecycle
```
SessionManager.create_with_credentials()
  → SshClient.connect_password/connect_key()
  → SshClient.open_shell()
  → Session { input: mpsc::Sender, output: broadcast::Sender }
  → Frontend attaches via Tauri event listener
```

### Security Model
- Credentials are **encrypted at rest**: AES-256-GCM with a device-local key (`src-tauri/src/storage/crypto.rs`); encrypted values carry an `enc:v1:` prefix and a startup migration encrypts legacy plaintext rows
- SSH host-key verification is **TOFU, enforced server-side** in the connect flow: `probe_host_key` → user approval → connect; a changed host key hard-fails before authentication
- Terminal output events are **base64-encoded and coalesced** — the frontend must base64-decode event payloads before writing to xterm
- The service↔GUI IPC directory is per-user **0700**, with a private socket. GUI startup owns the service when available or attaches to an existing one. Route each operation to its session's owner.
- MCP transport is plain HTTP bearer tokens, accepted for a trusted internal network by explicit operator opt-in. Pairing is a single-use six-digit code (300 s TTL, five-attempt limit), tokens are stored as SHA-256 hashes, and revocation/rotation take effect on the next request. A LAN listener refuses wildcard addresses and will not start with zero paired Agents.
- Paired Agents are limited by a tool allowlist plus session ownership, not only by prompt policy. `is_agent_tool`/`get_agent_tool_definitions` in `src-tauri/src/mcp/tools.rs` and the paired-Agent branch in `handle_tools_call` must stay in sync; `session_kill` rejects the batch form because ownership of one session must not grant authority over all of them.
- Sudo is injected by the service only when the site has both a stored sudo credential and an exact command in `sudo_policies`; anything else fails closed.
- Tests must use injected temporary databases/keys. Never call production data initializers in test helpers or run all ignored tests against user configuration.
- Credentials must never enter Debug/activity output. Preserve exact secret bytes and atomic metadata/credential updates.

### Theme System
- CSS variables set on `document.documentElement` from `settingsStore`
- Tailwind classes: `bg-tokyo-bg`, `text-tokyo-fg`, `border-tokyo-bg-hl`, etc.
- **NEVER** use hardcoded hex colors — always use `tokyo-*` Tailwind utilities

## Project Structure

```
src/                          # Frontend
  ├── components/             # React components (each in own folder)
  ├── stores/                 # Zustand stores
  ├── lib/                    # Utilities (tauri.ts, utils.ts)
  ├── types/                  # TypeScript type definitions
  ├── App.tsx                 # Main layout
  └── styles.css              # Global styles + Tailwind

src-tauri/                    # Rust backend
  ├── src/
  │   ├── commands/           # Tauri command handlers
  │   ├── ssh/                # SSH client (russh)
  │   ├── sftp/               # SFTP operations
  │   ├── session/            # Session + SessionManager
  │   ├── tunnel/             # Tunnel runtime retained for session cleanup only (no UI/commands)
  │   ├── logging/            # Session recording
  │   ├── storage/            # Database + models + credential/sudo crypto (crypto.rs)
  │   ├── local_shell/        # Local terminal (portable-pty)
  │   ├── ipc/                # Service↔GUI IPC socket (per-user 0700)
  │   ├── mcp/                # MCP service: HTTP JSON-RPC, pairing, Agent tool surface
  │   ├── install/            # AI tool skill installer
  │   ├── cloud_sync/         # Encrypted cloud sync vault
  │   ├── dbconn/             # Database connection management
  │   ├── platform/           # Platform/window integration and app-data locations
  │   ├── plugins/            # Plugin runtime (no marketplace UI or Agent tools)
  │   └── lib.rs              # App entry, command registration
  └── Cargo.toml

cli/                          # zxyssh binary: runs and administers the MCP service only

plugins/                      # Built-in plugin catalog (workspace member: zxyssh-plugins)
├── src/lib.rs                # Plugin spec: manifest types, validation, command rendering
└── builtin/<plugin-id>/plugin.json   # One directory per built-in plugin
```

**Plugin spec:** `docs/plugin-spec.md` is normative. Built-in plugins live in `plugins/builtin/<id>/plugin.json` and must be registered in `BUILTIN_MANIFESTS` (plugins/src/lib.rs); a test enforces directory ↔ registration parity. The catalog, its reference documents and `check-release.mjs` parity remain enforced, but plugin actions are not exposed to the CLI or to paired Agents; changing that requires new tool-surface work, not a docs change.

## Database Schema (SQLite)

| Table | Purpose |
|-------|---------|
| `servers` | SSH server configs (host, port, auth, jump_host, post_login_cmd) |
| `groups` | Server organization groups |
| `credentials` | Device-local credential storage (encrypted at rest, see Security Model) |
| `server_credentials` | Per-server saved credentials |
| `sudo_credentials` | Per-server sudo passwords (encrypted, service-side injection only) |
| `sudo_policies` | Per-server exact allowlist of non-interactive sudo commands |
| `recordings` | Session recording metadata |
| `settings` | App settings (key-value) |
| `agent_activity` | Durable Agent/MCP operation history |
| `tunnel_configs`, `command_snippets`, `command_history`, `plugin_installations` | Legacy tables kept for existing installations; no current UI writes them |

## Stores (Frontend State)

Stores in `src/stores/` (`cloudSyncCoordinator.ts` is a non-store orchestrator and is not listed):

| Store | Purpose |
|-------|---------|
| `agentApprovalStore` | Approvals for dangerous AI-agent commands (Agent Gateway) |
| `agentActivityStore` | Durable operation history, incremental recovery and pagination |
| `cloudSyncStore` | Cloud sync provider pairing/vault state |
| `dbConnectionsStore` | Database connection profiles |
| `fileWorkspaceStore` | Open local/SFTP files + viewer state |
| `fingerprintStore` | SSH host key verification |
| `localShellStore` | Local terminal sessions |
| `navigationStore` | View routing (main/settings) |
| `notificationStore` | Toast notifications |
| `recordingStore` | Session recording state |
| `runtimeCapabilitiesStore` | Runtime capability flags (platform, local shell, agent gateway, updater) |
| `serverStore` | Server/group CRUD |
| `sessionStore` | SSH session lifecycle |
| `settingsStore` | App settings + themes + AI tool config |
| `updateStore` | App update check/download state |

`tunnelStore.ts` still exists but has no importers; the tunnel UI and its Tauri commands were removed.

## Tauri Commands

Organized by module under `src-tauri/src/commands/`: `session`, `server`, `sftp`, `fingerprint`, `local_shell`, `local_shell_mobile`, `logging`, `install`, `dialog`, `settings`, `cloud_sync`, `dbconn`, `plugin`, `local_files`, `workspace_window`, `agent`, `app`, `platform`.

All registered in `src-tauri/src/lib.rs` → `invoke_handler`. The `snippet`, `history`, `tunnel` and `coding_agent` command modules were removed with their UI; do not re-register them to fix a missing frontend call.

## Build & Run

```bash
# Development
npm run dev              # Start Vite dev server
npx tauri dev            # Start Tauri dev (frontend + backend)

# Production build
npm run build            # TypeScript + Vite build
unset CI CXX CC          # Required on some systems
npx tauri build --no-bundle  # Compile release exe

# Rust only
cd src-tauri && cargo check  # Type check
cd src-tauri && cargo test   # Run tests
```

**Important:** On Windows, `unset CI CXX CC` may be needed before `tauri build` to avoid `cc-rs` toolchain detection issues.

## Testing

- **Rust tests:** `cargo test` from the repo root (single Cargo workspace: `src-tauri`, `cli`, `plugins`) — unit tests live inside modules (`storage`, `ssh`, `sftp`, `mcp`, `ipc`, `install`, `local_shell`, …), integration tests in `src-tauri/tests/`
- **SSH integration tests:** run `bash scripts/test-ssh-compatibility.sh` for a generated-key, loopback-only Docker fixture. Other ignored tests require their own explicit external fixtures; never run them wholesale against saved user data.
- **LAN MCP gateway:** `bash scripts/test-lan-mcp-gateway.sh [address]` drives a real `zxyssh mcp serve` listener (pairing, bearer auth, body limit, audit-at-rest, revocation) inside a throwaway state directory under `target/`; it never opens the operator's database. Network tests stay in `src-tauri/src/mcp/lan_test.rs` (`cfg(test)`) because `Crypto::device()` only yields a deterministic test key there.
- **Frontend tests:** `npm test` (Vitest, see `vitest.config.ts`) — `*.test.ts` files colocated with stores/components; CI runs them
- **Formatting:** `cargo fmt --check` gates CI — run `cargo fmt` before committing Rust changes

Pre-completion gates remain `npm run build` + `cargo check` (see When Making Changes).

## Code Style

- **Rust:** Standard Rust 2021 edition, `snake_case`, `log` crate for logging
- **TypeScript:** Strict mode, ES modules, React functional components with hooks
- **CSS:** Tailwind utility-first with Tokyo Night custom theme classes
- **State:** Zustand with `safeInvoke` pattern for all backend calls
- **Error handling:** `anyhow::Result` in Rust, `InvokeResult<T>` discriminated union in TS

## When Making Changes

1. **New Tauri command:** Add to `src-tauri/src/commands/`, export in `mod.rs`, register in `lib.rs` invoke_handler
2. **New frontend store:** Create in `src/stores/`, use `safeInvoke` pattern
3. **New component:** Create folder in `src/components/`, use `tokyo-*` theme classes
4. **New DB table:** Add `CREATE TABLE IF NOT EXISTS` in `database.rs::init_schema()`, add model in `models.rs`
5. **Always:** Run `npm run build` + `cargo check` before considering work complete
