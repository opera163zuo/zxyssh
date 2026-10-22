<div align="center">
  <img src="app-icon.svg" width="96" alt="zxySSH" />
  <h1>zxySSH</h1>
  <p><strong>Keep your servers, files, and AI work in the same place.</strong></p>
  <p>An SSH terminal you can work in yourself, share with an agent, and make your own.</p>

  [English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

  [![Manual CI](https://github.com/opera163zuo/zxyssh/actions/workflows/ci.yml/badge.svg?branch=dev&event=workflow_dispatch)](https://github.com/opera163zuo/zxyssh/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/opera163zuo/zxyssh)](https://github.com/opera163zuo/zxyssh/releases)
  [![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

  [Download](https://github.com/opera163zuo/zxyssh/releases) · [Agent / CLI guide](skills/zxyssh/SKILL.md) · [Changelog](CHANGELOG.md) · [Contribute](CONTRIBUTING.md)
</div>

![Terminal, host status and visible agent operations in one zxySSH workspace](docs/assets/screenshots/tour-collaboration.png)

*Real zxySSH components, synthetic Northstar demo data. The gallery uses an isolated browser fixture: no live servers, credentials, model calls, or service restarts. Agent transcripts and command results are examples, not recordings of an actual agent run. [Reproduce the screenshots](scripts/readme-demo/README.md).*

## Less passing context between tools

A routine server task rarely stays in one terminal. You check a log, find a configuration file, open an editor, ask an agent for help, then work out which machine and session each tool is using.

zxySSH keeps that work together. SSH and local terminals share a tabbed workspace with the remote file panel and the local file editor. Use it as a normal terminal; an AI Agent reaches the same hosts through the MCP service rather than through your keyboard. You do not need a model account for ordinary terminal and file work.

The difference is the workflow, not a new SSH protocol. OpenSSH, tmux, editors and scripts can cover many of the same jobs. zxySSH reduces the setup, copying and window switching needed to make them work together.

## Let an agent help without losing the thread

### See what it ran, and where

Desktop and MCP use the same saved servers, and an Agent works inside the sessions it created. When an Agent runs a command, the activity strip and history show the tool, session, time and operation state. Repeated attempts remain separate records; multiline input is not reduced to a one-line label.

Two collaboration shapes remain. Shared input goes into an interactive shell you are both looking at; independent execution lets an Agent inspect a host without typing over your work. Both are visible in history, and “input sent” stays separate from “command completed.”

An agent-created session appears as another tab without switching away from the tab you selected. Two connections to the same server remain two sessions, not one ambiguous server-name tab. History can be paged through and recovered after reopening the UI.

### Approve the actual operation, not a vague request

The approval dialog puts the proposed command and the reasons for review in front of you. You can allow that operation or reject it instead of discovering a service restart in the transcript afterward. MCP tool calls keep the same permission and confirmation checks, and a transport with no approval surface fails closed.

![Actual agent approval dialog showing the proposed restart and reasons for review](docs/assets/screenshots/tour-agent-approval.png)

*Demo request only. The service restart in this image was never executed. Command classification and approval are useful controls, not a sandbox or a guarantee that every risky command will be recognized.*

## A terminal that helps with the small things

You should not need AI just to remember a flag. Built-in completion suggests commands, subcommands and options with descriptions, and keeps the answer near the cursor through inline suggestions and a keyboard-driven list.

For an extra nudge, enable **AI command prediction** with your own OpenAI-compatible or Claude endpoint and model. It proposes a suffix to what you are typing; it does not run the suggestion for you. This is separate from the MCP Agent surface.

**Prediction is off by default.** When enabled, the current input and local completion candidates are sent to the provider you configure. Leave it disabled for work that must not leave the machine; ordinary local completion remains available.

Terminal rendering uses xterm.js, a WebGL renderer where available, and batched input/output paths to reduce UI overhead. These are responsiveness choices, not a claim that zxySSH makes the SSH network faster.

## Find a connection without remembering an address

The site tree and connection launcher bring **SSH and local shells** into one place. Search your saved servers, switch between compact lists and cards, and organize targets with nested groups. A status dot marks the sites that already have a session, and a separate new-session action helps distinguish “return to my work” from “open another connection.”

![Searchable connection cards with groups, saved targets and existing-session indicators](docs/assets/screenshots/tour-connections.png)

Configure a jump host for private targets and add a site with its credential in the same dialog. The OpenSSH, PuTTY and Tabby profile readers remain in the codebase for migration tooling, but no import entry point is currently exposed: import those profiles from their own tools, or restore them from a zxySSH sync export. Stored passwords from other applications are deliberately not copied, and PuTTY `.ppk` keys need conversion to OpenSSH format.

Changing a saved password, private key or passphrase does not require deleting the server. Untouched fields keep their values, existing secrets are not loaded into the edit form, and profile/credential changes save together or roll back together. This changes **zxySSH's saved login information**, not the remote account's password.

## Files belong next to the command that uses them

Open a remote configuration through SFTP, or use **⌘/Ctrl+O** for local files without opening an SSH connection at all. Documents get their own tabs, so closing the last terminal does not close your local notes.

The file workspace offers editable text/code, syntax highlighting, and Markdown source, preview, or both side by side. SFTP includes column/icon browsing, multiple selection, path copying, upload/download progress, and viewers for supported images, PDFs, media and archives. Read a runbook, inspect a log and edit a config without maintaining a separate mental map of windows.

Move file and terminal panes around, split the workspace, or detach a document into another window. Unsaved text stays with its editing buffer during layout changes. Local text saves detect changes made by another program and refuse to silently overwrite them; truncated reads are not offered as a full-file save.

Folder transfer is also about correctness: bounded chunks, downloads that wait for local writes, content comparison for same-size edits, and protection for excluded paths and nested `.gitignore` rules when deleting extras. Content comparison can add remote reads; it is not a promise of lower bandwidth.

[Local files, Markdown behavior and editing limits](docs/local-files-and-css-themes.md)

## A terminal, or an Agent through MCP

The desktop is where a person works: connect from the site tree, run commands, transfer files. The same SSH capability is also exposed to AI Agents through the MCP service, so an Agent on another machine can drive a session without ever holding a password.

Paired Agents are limited on purpose. They see the site list and the terminal input and output of the sessions they created:

| What an Agent can do | How the service protects you |
| --- | --- |
| List sites, open a session | Sessions are created by saved site name, and each Agent gets its own session |
| Send keystrokes, read output, resize | An Agent can only touch sessions it created |
| Run a privileged command | The sudo password and an exact command allowlist are configured by a person; the Agent never sees the secret |
| Close its own session | Batch termination is refused for Agents |

Anything beyond that — arbitrary `exec`, remote file search, SFTP upload or download, editing server records — stays out of the paired Agent surface. Operations appear in the desktop activity panel, and commands that need approval fail closed when no approval UI is attached.

[Agent collaboration model](docs/AGENT_COLLABORATION.md) · [Main Skill reference](skills/zxyssh/SKILL.md)

## Arrange it around the way you work

**Keep context, not a pile of windows.** Terminals and documents can be split, rearranged and moved into separate windows. Save a layout and return to it; restoring the workspace is not a promise that an old network connection survives a process restart.

**Make long sessions comfortable.** Choose light or dark themes, follow the system appearance, adjust terminal fonts and cursors, and use keyboard navigation or reduced-motion settings. The application has English and Simplified Chinese UI; the Japanese README is a documentation translation, not a claim of Japanese UI support.

**Go beyond a color preset.** The custom CSS editor supports live preview, apply/save, import/export and local background images. Change spacing, corners and document typography as well as colors. If a theme hides the controls, **⌘/Ctrl+Shift+F12** or the native *Disable Custom CSS* menu can turn it off. Only apply trusted CSS: remote URLs in a theme can make network requests.

[Custom CSS guide and recovery](docs/local-files-and-css-themes.md) · [Starter theme](themes/vibecode-starter.css)

## The SSH essentials are still here

Host identity is checked before authentication, including the real destination behind a jump host, and a changed fingerprint stops the connection instead of warning its way through. Session recording can be turned on and its files are listed and deletable from Settings; the current interface does not ship a playback player. Credentials, sudo passwords and agent activity use encrypted local storage. That is not OS Keychain custody or protection from a compromised local account. A correct private key does not justify accepting an unexpected server fingerprint.

Optional encrypted **Gist/WebDAV sync** carries server metadata and groups between your own setups. It is not a zxySSH-hosted SSH relay. Login credentials, host-key trust, live terminal sessions and agent activity history stay outside that sync. Protect provider tokens and recovery material.

## Linux server and cross-platform agents

The Linux-hosted service targets Ubuntu/Debian servers. Remote Agents can connect from any operating system with MCP HTTP support. zxySSH does not provide macOS or Windows desktop or native CLI packages.

[Linux server installation and LAN MCP setup](docs/LAN-MCP-UBUNTU-DEBIAN.md) · [Standalone CLI installation](cli/README.md)

The `zxyssh` binary is the service and its administration surface. It runs the listener, mints pairing codes, lists and revokes Agents, handles headless host-key trust, and stores credentials on the host:

```bash
zxyssh mcp serve                        # run the listener (systemd Type=simple)
zxyssh mcp pair-code                    # one-time code for the next Agent
zxyssh mcp requests list                # pairing requests waiting for approval
zxyssh mcp requests approve <request-id>
zxyssh mcp approvals                    # whether pairing waits for approval
zxyssh mcp agents                       # paired Agents and revocation state
zxyssh mcp credential set my-server     # read the secret from piped stdin
zxyssh mcp hostkey probe my-server      # headless TOFU before a connection
```

A correct pairing code only opens a request: the credential is issued after an operator approves it in the desktop or with `zxyssh mcp requests approve`, and it comes back sealed to the Agent's ephemeral key, so a pairing request must present `clientPublicKey` while the queue is on. A LAN listener needs a concrete bind address and an explicit opt-in, and refuses to start while no Agent is paired. Never put passwords or private keys in command-line arguments or Agent prompts.

The desktop and the service share one database when they run as the same user, so sites added in the UI are the sites an Agent can use. The process owning a connection must remain alive: service-owned sessions can outlive the GUI, while GUI-owned ones end when that GUI exits. Upgrade the desktop and CLI together, and save ongoing work before restarting.

Mobile targets remain experimental. Common OpenSSH paths are tested, but not every MFA flow, hardware token or SSH implementation.

## Build, contribute, or explore the implementation

Built with Tauri 2, Rust, React, TypeScript and xterm.js. Use Node.js 22.12+, current stable Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone --branch dev https://github.com/opera163zuo/zxyssh.git
cd zxyssh
npm ci
npm run tauri -- dev
```

Before a PR: `node scripts/check-release.mjs`, `npm test`, `npm run build`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`. SSH changes also have a loopback-only Docker fixture: `bash scripts/test-ssh-compatibility.sh`. Never run credential tests against a real saved-server store.

**All normal PRs target `dev`.** `main` receives tested release promotions from this repository's `dev`. Build commands do not authorize replacing an installed app or ending someone's SSH sessions.

CI builds and releases are **manual-only**. Maintainers run CI on the PR head before merging; pushing code or a version tag does not compile or publish. Release runs from `main` and defaults to a draft unless publication is explicitly selected. Required checks, signatures and complete-source validation remain in place.

Docker inventory improvements and the remaining container-session/proxy work are tracked in the [September issue review](docs/ISSUE_TRIAGE_2026-09.md). A container listing command is not a persistent container session; a SOCKS forwarding listener is not an outbound proxy setting.

[Contributing](CONTRIBUTING.md) · [Architecture](AGENTS.md) · [Release process](docs/RELEASING.md) · [Collaboration API](docs/AGENT_COLLABORATION.md)

Report security concerns through [private security reporting](https://github.com/opera163zuo/zxyssh/security/advisories/new), not with real credentials in an issue. Approval controls are not a sandbox; protected input cannot prevent a remote program from echoing it. Do not automatically replay a mutating command after an ambiguous response loss.

## License

zxySSH has been distributed as a whole under **GPL-3.0-only** since 1.1.0. See [LICENSE](LICENSE), [NOTICE](NOTICE) and the [preserved MIT notice](licenses/legacy-MIT.txt). Earlier MIT permissions are not revoked; third-party components keep their own licenses. Release downloads include corresponding source and notices. No warranty is provided to the extent permitted by law.
