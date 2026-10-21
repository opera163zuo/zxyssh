# zxySSH LAN MCP on Ubuntu/Debian

Run zxySSH's MCP endpoint as a headless service on one Linux server. The recommended deployment runs the MCP process as the same Linux user as the desktop UI, so the site tree, encrypted credentials, host-key fingerprints and database are shared. Agents on any operating system connect to it over standard MCP HTTP and operate the SSH sessions owned by this host.

A separate system-user unit is still available for an isolated headless deployment, but it intentionally uses a different database and device key; it does not share sites created by the desktop user's UI.

```
Hermes / any MCP client  ──HTTP + Bearer token──▶  zxyssh mcp serve  ──SSH──▶  targets
   (macOS, Windows, Linux)                          (same Linux user as zxySSH)
```

Verified on Debian 13 with an isolated loopback OpenSSH container: pairing,
`initialize`, `tools/list`, `server_add`, key-based `session_create`, `exec`,
`sftp_ls`, fail-closed approval, and immediate revocation.

## 1. Install the same-user service

```bash
cargo build --release --locked -p zxyssh
install -Dm0755 target/release/zxyssh "$HOME/.local/bin/zxyssh"
install -Dm0644 deploy/systemd/zxyssh-mcp.user.service \
  "$HOME/.config/systemd/user/zxyssh-mcp.service"
install -Dm0600 /dev/null "$HOME/.config/zxyssh/mcp.env"
```

The user unit runs as your logged-in Linux user; it has no separate `User=`,
`StateDirectory=` or database. It and the desktop app therefore use the same
`$HOME/.local/share/com.zxyssh.desktop/zxyssh.db` and device key. The unit
sets `XDG_DATA_HOME` to `%h/.local/share`, matching the desktop default. If the
desktop is launched with a custom `XDG_DATA_HOME`, update the unit to the same
value before starting it.

## 2. Configure

Edit `~/.config/zxyssh/mcp.env`:

```ini
# Loopback only is the default; use a private interface only on a trusted LAN.
ZXYSSH_MCP_BIND=127.0.0.1:8765
```

The file is mode `0600`; do not put passwords or bearer tokens in it. SSH
credentials are saved by the desktop UI and encrypted in the shared database.

The separate system unit `deploy/systemd/zxyssh-mcp.service` runs as the
`zxyssh` system user with state under `/var/lib/zxyssh`. It is an isolated
headless deployment: it does **not** share desktop sites or credentials. Use it
only when that separation is intended, and provision its site database and
credentials independently.

## 3. Start and pair

```bash
systemctl --user daemon-reload
systemctl --user enable --now zxyssh-mcp.service
curl -s http://127.0.0.1:8765/health

~/.local/bin/zxyssh mcp pair-code
# One-time pairing code is printed locally; valid for five minutes and one use.
```

To keep the user service running after logout, enable lingering for this account:

```bash
sudo loginctl enable-linger "$USER"
```

The Agent exchanges the code for its own bearer token. The token is returned once; only its hash is stored. Pair from the trusted private network; this service uses HTTP, so do not send bearer tokens across an untrusted network.

Point the Agent at `http://127.0.0.1:8765/mcp` using the issued token in the `Authorization: Bearer …` header. Every MCP route requires that header; only `/health` and `/pair` are unauthenticated.

```bash
~/.local/bin/zxyssh mcp agents
~/.local/bin/zxyssh mcp revoke <agent-id>
~/.local/bin/zxyssh mcp rotate <agent-id>
```

Revocation and rotation take effect on the Agent's next request — the service
re-reads the registry from storage, it does not rely on a restart.

## 4. Let the Agent reach a real host

Three steps, each on the side that owns the decision:

1. **Server profile** — create/edit it in the zxySSH site UI. The LAN Agent tool surface is read-only for sites; it can select a site by name but cannot add, edit or delete profiles.
2. **Credential** — save the SSH password or private key in the UI. For headless provisioning, the administrator may pipe a secret to the CLI; it is encrypted with the same device key and is never accepted as a command-line argument.

   ```bash
   ~/.local/bin/zxyssh mcp credential set prod-web --auth-type password < /path/to/prod-web-password
   ~/.local/bin/zxyssh mcp credential set prod-web --auth-type key < ~/.ssh/id_ed25519
   ```
3. **Host key** — the desktop UI asks for TOFU approval during manual login. For a headless deployment, verify the fingerprint out of band before pinning it:

   ```bash
   ~/.local/bin/zxyssh mcp hostkey probe prod.example --port 22
   ~/.local/bin/zxyssh mcp hostkey trust prod.example --port 22 --fingerprint SHA256:…
   ```

Then the Agent calls `session_create` with the saved site name, followed by `session_send_input`, `session_read` and `session_resize`. The LAN surface does not expose arbitrary `exec`, SFTP, local-file upload or site mutation tools. If a sudo password is configured for the site, `session_sudo` runs a non-interactive command through a separate SSH exec channel; the password never enters the Agent request, PTY input, audit summary or response. A command that requires an interactive TTY or password change fails closed.

## 5. Verify before exposing it

`scripts/test-lan-mcp-gateway.sh` re-checks the gateway against a throwaway state
directory under `target/`. It never reads your saved servers, credentials or
paired Agents, and it contacts no SSH host.

```bash
bash scripts/test-lan-mcp-gateway.sh                  # free port, detected IPv4
bash scripts/test-lan-mcp-gateway.sh 192.168.1.20     # name the interface address
```

It asserts 18 checks in four groups:

1. **Boot guard** — a non-loopback bind with no paired Agent must refuse to start.
2. **Transport and auth** — health, `401` with no token and with a forged one,
   one-time code exchange plus a rejected replay, `initialize`, `202` with an empty
   body for notifications, `tools/list`, a real `tools/call`, `-32601` for an
   unknown method, `413` at the body limit, and a service log containing neither
   the pairing code nor the issued token.
3. **Audit at rest** — every `agent_activity` row written by the run carries the
   `enc:v1:` prefix.
4. **LAN and revocation** — the listener opens on the named address only because an
   Agent is already paired, a second Agent pairs over that address, and a revoked
   token is rejected on its very next request without restarting the service.

Re-run it after an upgrade or a unit change; failing here is cheaper than failing
on a live Agent. The address argument is positional, and the LAN group is skipped
with a notice when the host has no global IPv4 address.

## 6. Reaching beyond loopback

The service reads bind settings from `~/.config/zxyssh/mcp.env`; the reverse proxy, if used, runs on the same host and terminates HTTPS before forwarding to `127.0.0.1:8765`. The service itself uses HTTP. In the requested trusted-LAN deployment, restrict the port with firewall rules and do not expose it to public or untrusted networks.

Four rules are enforced at start-up (`validate_listener`), so a default or a typo can never expose the port:

- a loopback bind is always allowed;
- any other address needs `ZXYSSH_MCP_ALLOW_LAN=true` in the environment file;
- `0.0.0.0` and `::` are refused — name one interface address, e.g. `ZXYSSH_MCP_BIND=192.168.1.20:8765`;
- a non-loopback bind with zero active paired Agents is refused, so pairing has to happen first.

`bash scripts/test-lan-mcp-gateway.sh` covers the first and last of these; restrict which hosts may connect with your own firewall rules.

## 7. Backups

For the same-user deployment, back up `$HOME/.local/share/com.zxyssh.desktop` as one secret; it contains `zxyssh.db`, the `ssh_credentials.key`/`.salt` device key files and `ssh_fingerprints.json`. Encrypted rows are unreadable without the device key, and the key is useless without the database. Keep the directory private and offline. The dedicated system-user unit instead stores its independent state under `/var/lib/zxyssh/com.zxyssh.desktop`.

```bash
systemctl --user stop zxyssh-mcp.service
tar --mode='go-rwx' -czf "$HOME/zxyssh-state.tar.gz" -C "$HOME/.local/share" com.zxyssh.desktop
systemctl --user start zxyssh-mcp.service
```

## 8. What this service does not do

- **No self-approval.** Risky commands are gated by the human approval UI, which
  a headless process does not have. Such a call fails closed with
  `Command requires user approval, but no approval UI is available.` Read-only
  and non-flagged operations run normally.
- **No shared isolation between Agents.** They share one database, but each paired HTTP Agent can only use sessions it created through the LAN tool surface. Two Agents are still not full tenants; saved sites and the configured sudo capability are common to the service.
- **No arbitrary root shell guarantee.** `session_sudo` is an explicit configured-password operation; any Agent with the token can request the configured site's sudo capability. Do not configure it unless that is intended, and do not treat it as protection from a command running as root on the target host.
- **No TLS in the process.** Use a proxy or a private network, as above.
- **No browser-origin checks.** The endpoint is for native MCP clients; requests
  carrying an `Origin` header are not validated, so do not expose it to browsers.
- **No per-Agent server allowlists yet**; scoping is the common permission profile.
