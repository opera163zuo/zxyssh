# Git Workspace — git-workspace

Plugin `git-workspace` version `1.1.0`.

Inspect repository status, branches and recent commits in the remote login directory.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `status`

Show branch and working tree state.

```json
{
  "allowSudo": false,
  "description": "Show branch and working tree state.",
  "elevate": false,
  "id": "status",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Status",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `branches`

Show local branches and upstream tracking.

```json
{
  "allowSudo": false,
  "description": "Show local branches and upstream tracking.",
  "elevate": false,
  "id": "branches",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Branches",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `history`

Show the latest 30 commits.

```json
{
  "allowSudo": false,
  "description": "Show the latest 30 commits.",
  "elevate": false,
  "id": "history",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Recent history",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## Running this from the MCP service

The networked MCP endpoint gives a paired Agent only `server_list`, `session_create`, `session_send_input`, `session_sudo`, `session_read`, `session_resize` and `session_kill`. Plugin actions are not part of that surface, and neither are `exec`, remote `rg` or SFTP transfer, so there is no supported way to invoke this plugin remotely today. Do not substitute a raw command through `session_send_input` to reach it: that skips the installation, permission and confirmation checks this manifest declares. A model-supplied confirmation flag is never approval, and SSH host-key checks remain enforced.
