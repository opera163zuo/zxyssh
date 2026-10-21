# Docker Containers — docker-containers

Plugin `docker-containers` version `1.5.0`.

Inspect and manage containers, images, live resource usage and recent container logs through the remote Docker CLI.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `running-containers`

List running containers with full IDs; paused and restarting containers are not shell-ready.

```json
{
  "allowSudo": true,
  "description": "List running containers with full IDs; paused and restarting containers are not shell-ready.",
  "elevate": false,
  "id": "running-containers",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Running containers",
  "output": {
    "columns": [
      "ID",
      "Name",
      "Image",
      "State",
      "Status",
      "Ports"
    ],
    "delimiter": "\t",
    "kind": "table"
  },
  "requiresConfirmation": false
}
```

## `exited-containers`

Inspect exited containers separately without starting or restarting them.

```json
{
  "allowSudo": true,
  "description": "Inspect exited containers separately without starting or restarting them.",
  "elevate": false,
  "id": "exited-containers",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Exited containers",
  "output": {
    "columns": [
      "ID",
      "Name",
      "Image",
      "State",
      "Status",
      "Ports"
    ],
    "delimiter": "\t",
    "kind": "table"
  },
  "requiresConfirmation": false
}
```

## `container-inventory`

Read all container states and full IDs as one JSON object per line, not a JSON array. This does not create a container session.

```json
{
  "allowSudo": true,
  "description": "Read all container states and full IDs as one JSON object per line, not a JSON array. This does not create a container session.",
  "elevate": false,
  "id": "container-inventory",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Container inventory (JSON Lines)",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `containers`

List running and stopped containers.

```json
{
  "allowSudo": true,
  "description": "List running and stopped containers.",
  "elevate": false,
  "id": "containers",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Containers",
  "output": {
    "columns": [
      "ID",
      "Name",
      "Image",
      "Status",
      "Ports"
    ],
    "delimiter": "\t",
    "kind": "table"
  },
  "requiresConfirmation": false
}
```

## `stats`

Capture one resource-usage snapshot for running containers.

```json
{
  "allowSudo": true,
  "description": "Capture one resource-usage snapshot for running containers.",
  "elevate": false,
  "id": "stats",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Live snapshot",
  "output": {
    "columns": [
      "Container",
      "CPU",
      "Memory",
      "Network",
      "Block I/O"
    ],
    "delimiter": "\t",
    "kind": "table"
  },
  "requiresConfirmation": false
}
```

## `images`

List locally available Docker images.

```json
{
  "allowSudo": true,
  "description": "List locally available Docker images.",
  "elevate": false,
  "id": "images",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Images",
  "output": {
    "columns": [
      "Image",
      "ID",
      "Size",
      "Created"
    ],
    "delimiter": "\t",
    "kind": "table"
  },
  "requiresConfirmation": false
}
```

## `logs`

Read the latest 200 log lines from a container.

```json
{
  "allowSudo": true,
  "description": "Read the latest 200 log lines from a container.",
  "elevate": false,
  "id": "logs",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "container"
    ],
    "type": "object"
  },
  "name": "Container logs",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `inspect`

Return Docker's structured container metadata.

```json
{
  "allowSudo": true,
  "description": "Return Docker's structured container metadata.",
  "elevate": false,
  "id": "inspect",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "container"
    ],
    "type": "object"
  },
  "name": "Inspect container",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `exec-command`

Run one non-interactive shell command inside a container. This does not create a persistent container session or change the host session.

```json
{
  "allowSudo": true,
  "description": "Run one non-interactive shell command inside a container. This does not create a persistent container session or change the host session.",
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
      }
    },
    "required": [
      "container",
      "command"
    ],
    "type": "object"
  },
  "name": "Run in container",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `start-container`

Start a stopped container.

```json
{
  "allowSudo": true,
  "description": "Start a stopped container.",
  "elevate": false,
  "id": "start-container",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "container"
    ],
    "type": "object"
  },
  "name": "Start container",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `stop-container`

Stop a running container gracefully.

```json
{
  "allowSudo": true,
  "description": "Stop a running container gracefully.",
  "elevate": false,
  "id": "stop-container",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "container"
    ],
    "type": "object"
  },
  "name": "Stop container",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `restart-container`

Restart a container.

```json
{
  "allowSudo": true,
  "description": "Restart a container.",
  "elevate": false,
  "id": "restart-container",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "container"
    ],
    "type": "object"
  },
  "name": "Restart container",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `volumes`

List Docker volumes and their drivers.

```json
{
  "allowSudo": true,
  "description": "List Docker volumes and their drivers.",
  "elevate": false,
  "id": "volumes",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Volumes",
  "output": {
    "columns": [
      "Volume",
      "Driver",
      "Scope"
    ],
    "delimiter": "\t",
    "kind": "table"
  },
  "requiresConfirmation": false
}
```

## `networks`

List Docker networks.

```json
{
  "allowSudo": true,
  "description": "List Docker networks.",
  "elevate": false,
  "id": "networks",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Networks",
  "output": {
    "columns": [
      "Network",
      "Driver",
      "Scope"
    ],
    "delimiter": "\t",
    "kind": "table"
  },
  "requiresConfirmation": false
}
```

## `version`

Show the Docker server version.

```json
{
  "allowSudo": true,
  "description": "Show the Docker server version.",
  "elevate": false,
  "id": "version",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "Docker version",
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
