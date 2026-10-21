# Kubernetes Pods — kubernetes-pods

Plugin `kubernetes-pods` version `1.1.0`.

Inspect cluster contexts, namespaces, pods, deployments and workload logs with kubectl.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `contexts`

Show available cluster contexts.

```json
{
  "allowSudo": false,
  "description": "Show available cluster contexts.",
  "elevate": false,
  "id": "contexts",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Contexts",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `pods`

List pods across all namespaces.

```json
{
  "allowSudo": false,
  "description": "List pods across all namespaces.",
  "elevate": false,
  "id": "pods",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Pods",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `deployments`

List deployments across all namespaces.

```json
{
  "allowSudo": false,
  "description": "List deployments across all namespaces.",
  "elevate": false,
  "id": "deployments",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Deployments",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `events`

Show recent cluster events.

```json
{
  "allowSudo": false,
  "description": "Show recent cluster events.",
  "elevate": false,
  "id": "events",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Recent events",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `pod-logs`

Read the latest 200 log lines from a pod.

```json
{
  "allowSudo": false,
  "description": "Read the latest 200 log lines from a pod.",
  "elevate": false,
  "id": "pod-logs",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "namespace": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "pod": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "namespace",
      "pod"
    ],
    "type": "object"
  },
  "name": "Pod logs",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `describe-pod`

Show pod status, conditions, mounts and recent events.

```json
{
  "allowSudo": false,
  "description": "Show pod status, conditions, mounts and recent events.",
  "elevate": false,
  "id": "describe-pod",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "namespace": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "pod": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "namespace",
      "pod"
    ],
    "type": "object"
  },
  "name": "Describe pod",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `exec-command`

Run one non-interactive shell command inside a selected Pod container.

```json
{
  "allowSudo": false,
  "description": "Run one non-interactive shell command inside a selected Pod container.",
  "elevate": false,
  "id": "exec-command",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "command": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "namespace": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "pod": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "namespace",
      "pod",
      "container",
      "command"
    ],
    "type": "object"
  },
  "name": "Run in Pod container",
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
