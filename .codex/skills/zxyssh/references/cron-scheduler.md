# Scheduled Tasks — cron-scheduler

Plugin `cron-scheduler` version `1.1.0`.

Manage cron jobs: browse the user crontab, /etc/crontab and /etc/cron.d, inspect systemd timers, and add or remove entries.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `crontab-list`

List the login user's crontab entries.

```json
{
  "allowSudo": true,
  "description": "List the login user's crontab entries.",
  "elevate": false,
  "id": "crontab-list",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "My crontab",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `timers`

List all systemd timers with their next and last runs.

```json
{
  "allowSudo": true,
  "description": "List all systemd timers with their next and last runs.",
  "elevate": false,
  "id": "timers",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "systemd timers",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `cron-d`

Show every file in /etc/cron.d with its contents.

```json
{
  "allowSudo": true,
  "description": "Show every file in /etc/cron.d with its contents.",
  "elevate": false,
  "id": "cron-d",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "cron.d drop-ins",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `crontab-file`

Read the system-wide crontab file.

```json
{
  "allowSudo": true,
  "description": "Read the system-wide crontab file.",
  "elevate": false,
  "id": "crontab-file",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "/etc/crontab",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `cron-add`

Append one entry to the login user's crontab.

```json
{
  "allowSudo": true,
  "description": "Append one entry to the login user's crontab.",
  "elevate": false,
  "id": "cron-add",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "line": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "line"
    ],
    "type": "object"
  },
  "name": "Add cron entry",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `cron-remove`

Remove one exact entry from the login user's crontab.

```json
{
  "allowSudo": true,
  "description": "Remove one exact entry from the login user's crontab.",
  "elevate": false,
  "id": "cron-remove",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "line": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "line"
    ],
    "type": "object"
  },
  "name": "Remove cron entry",
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
