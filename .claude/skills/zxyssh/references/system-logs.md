# System Logs — system-logs

Plugin `system-logs` version `1.1.0`.

Read recent systemd journal entries and service logs from the active server.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `recent`

Read the latest 200 journal entries.

```json
{
  "allowSudo": false,
  "description": "Read the latest 200 journal entries.",
  "elevate": false,
  "id": "recent",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Recent system log",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `service`

Read the latest 200 entries for one systemd unit.

```json
{
  "allowSudo": false,
  "description": "Read the latest 200 entries for one systemd unit.",
  "elevate": false,
  "id": "service",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "service": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "service"
    ],
    "type": "object"
  },
  "name": "Service log",
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
