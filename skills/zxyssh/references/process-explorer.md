# Process Explorer — process-explorer

Plugin `process-explorer` version `1.2.0`.

Inspect and manage running processes: sort by CPU or memory, inspect details, and terminate processes by PID.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `top-cpu`

Sort processes by current CPU usage.

```json
{
  "allowSudo": false,
  "description": "Sort processes by current CPU usage.",
  "elevate": false,
  "id": "top-cpu",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "CPU usage",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `top-memory`

Sort processes by current memory usage.

```json
{
  "allowSudo": false,
  "description": "Sort processes by current memory usage.",
  "elevate": false,
  "id": "top-memory",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Memory usage",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `process-detail`

Show full command line, working directory and start time of one PID.

```json
{
  "allowSudo": false,
  "description": "Show full command line, working directory and start time of one PID.",
  "elevate": false,
  "id": "process-detail",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "pid": {
        "description": "",
        "type": "integer"
      }
    },
    "required": [
      "pid"
    ],
    "type": "object"
  },
  "name": "Process detail",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `kill`

Send SIGTERM to a process by PID.

```json
{
  "allowSudo": true,
  "description": "Send SIGTERM to a process by PID.",
  "elevate": false,
  "id": "kill",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "pid": {
        "description": "",
        "type": "integer"
      }
    },
    "required": [
      "pid"
    ],
    "type": "object"
  },
  "name": "Terminate process",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## Running this from the MCP service

The networked MCP endpoint gives a paired Agent only `server_list`, `session_create`, `session_send_input`, `session_sudo`, `session_read`, `session_resize` and `session_kill`. Plugin actions are not part of that surface, and neither are `exec`, remote `rg` or SFTP transfer, so there is no supported way to invoke this plugin remotely today. Do not substitute a raw command through `session_send_input` to reach it: that skips the installation, permission and confirmation checks this manifest declares. A model-supplied confirmation flag is never approval, and SSH host-key checks remain enforced.
