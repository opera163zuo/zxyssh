# Redis Inspector — redis-inspector

Plugin `redis-inspector` version `1.2.0`.

Inspect and manage Redis: health, keyspace stats, key lookups, clients, slowlog and runtime configuration. Automatically uses a Docker redis container when redis-cli is not installed on the host.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `ping`

Check that the Redis server answers.

```json
{
  "allowSudo": true,
  "description": "Check that the Redis server answers.",
  "elevate": false,
  "id": "ping",
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
    "required": [],
    "type": "object"
  },
  "name": "Ping",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `info`

Read one INFO section from the Redis server.

```json
{
  "allowSudo": true,
  "description": "Read one INFO section from the Redis server.",
  "elevate": false,
  "id": "info",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "section": {
        "description": "",
        "enum": [
          "all",
          "server",
          "clients",
          "memory",
          "persistence",
          "stats",
          "replication",
          "cpu",
          "keyspace"
        ],
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "section"
    ],
    "type": "object"
  },
  "name": "Server info",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `keyspace`

Show per-database key counts and expiry stats.

```json
{
  "allowSudo": true,
  "description": "Show per-database key counts and expiry stats.",
  "elevate": false,
  "id": "keyspace",
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
    "required": [],
    "type": "object"
  },
  "name": "Keyspace",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `db-size`

Return the number of keys in the selected database.

```json
{
  "allowSudo": true,
  "description": "Return the number of keys in the selected database.",
  "elevate": false,
  "id": "db-size",
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
    "required": [],
    "type": "object"
  },
  "name": "Key count",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `scan-keys`

List keys matching a glob pattern.

```json
{
  "allowSudo": true,
  "description": "List keys matching a glob pattern.",
  "elevate": false,
  "id": "scan-keys",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "pattern": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "pattern"
    ],
    "type": "object"
  },
  "name": "Scan keys",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `get-key`

Read the string value stored under a key.

```json
{
  "allowSudo": true,
  "description": "Read the string value stored under a key.",
  "elevate": false,
  "id": "get-key",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "key": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "key"
    ],
    "type": "object"
  },
  "name": "Get key value",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `key-ttl`

Show the remaining time-to-live of a key in seconds (-1 forever, -2 missing).

```json
{
  "allowSudo": true,
  "description": "Show the remaining time-to-live of a key in seconds (-1 forever, -2 missing).",
  "elevate": false,
  "id": "key-ttl",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "key": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "key"
    ],
    "type": "object"
  },
  "name": "Key TTL",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `delete-key`

Delete one key from the database.

```json
{
  "allowSudo": true,
  "description": "Delete one key from the database.",
  "elevate": false,
  "id": "delete-key",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "key": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "key"
    ],
    "type": "object"
  },
  "name": "Delete key",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `clients`

List clients currently connected to the Redis server.

```json
{
  "allowSudo": true,
  "description": "List clients currently connected to the Redis server.",
  "elevate": false,
  "id": "clients",
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
    "required": [],
    "type": "object"
  },
  "name": "Connected clients",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `slowlog`

Show the 20 slowest recent commands.

```json
{
  "allowSudo": true,
  "description": "Show the 20 slowest recent commands.",
  "elevate": false,
  "id": "slowlog",
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
    "required": [],
    "type": "object"
  },
  "name": "Slow log",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `bigkeys`

Sample the dataset and report the largest key per type.

```json
{
  "allowSudo": true,
  "description": "Sample the dataset and report the largest key per type.",
  "elevate": false,
  "id": "bigkeys",
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
    "required": [],
    "type": "object"
  },
  "name": "Biggest keys",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `config-get`

Read one runtime configuration value.

```json
{
  "allowSudo": true,
  "description": "Read one runtime configuration value.",
  "elevate": false,
  "id": "config-get",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "setting": {
        "description": "",
        "enum": [
          "maxmemory",
          "maxmemory-policy",
          "appendonly",
          "appendfsync",
          "save",
          "databases",
          "timeout",
          "tcp-keepalive",
          "lazyfree-lazy-eviction"
        ],
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "setting"
    ],
    "type": "object"
  },
  "name": "Runtime config",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `key-type`

Show the value type stored under a key.

```json
{
  "allowSudo": true,
  "description": "Show the value type stored under a key.",
  "elevate": false,
  "id": "key-type",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "key": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "key"
    ],
    "type": "object"
  },
  "name": "Key type",
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
