# zxySSH MCP Service CLI

The `zxyssh` binary runs and administers the Linux MCP service. SSH site management and manual login are provided by the desktop UI; AI Agents connect through MCP HTTP and never receive saved credentials. zxySSH does not provide macOS or Windows desktop/CLI distributions.

## Install

```bash
./install.sh
```

The default destination is `~/.local/bin/zxyssh`. Override it with `ZXYSSH_INSTALL_DIR=/usr/local/bin ./install.sh` when appropriate.

## Run the MCP service

```bash
zxyssh mcp serve
```

The default listener is loopback at `127.0.0.1:8765`. For a same-user desktop deployment, install `deploy/systemd/zxyssh-mcp.user.service`; see `docs/LAN-MCP-UBUNTU-DEBIAN.md`. LAN binding requires an explicit private interface address, `ZXYSSH_MCP_ALLOW_LAN=true`, and an already paired Agent.

Pair and administer Agents with:

```bash
zxyssh mcp pair-code
zxyssh mcp agents
zxyssh mcp revoke <agent-id>
zxyssh mcp rotate <agent-id>
```

SSH passwords and private keys are saved through the desktop UI or provisioned by piping them to `zxyssh mcp credential set`. They are encrypted at rest and consumed by the Rust service; the MCP protocol never returns them. Sudo credentials and exact command allowlists are configured per site in the UI.

## Build from source

```bash
cargo build --release --locked --package zxyssh --bin zxyssh
```

To prepare the Linux target-suffixed binary consumed by Tauri desktop packaging:

```bash
node scripts/prepare-sidecar.mjs
```

zxySSH 1.1.0 is GPL-3.0-only. Release archives include LICENSE and NOTICE. The software comes without warranty to the extent permitted by law.
