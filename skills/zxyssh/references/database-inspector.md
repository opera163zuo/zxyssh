# Database Inspector — database-inspector

Plugin `database-inspector` version `1.3.0`.

Read PostgreSQL, MySQL and SQLite metadata. When the client binary is missing on the host, PostgreSQL and MySQL are queried inside their Docker containers.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["remote_exec","local_exec"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `postgres-databases`

List non-template PostgreSQL databases. Uses the host psql client, or a Docker postgres container when psql is absent.

```json
{
  "allowSudo": false,
  "description": "List non-template PostgreSQL databases. Uses the host psql client, or a Docker postgres container when psql is absent.",
  "elevate": false,
  "id": "postgres-databases",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [],
    "type": "object"
  },
  "name": "PostgreSQL databases",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `postgres-tables`

List user tables in a PostgreSQL database. Uses the host psql client, or a Docker postgres container when psql is absent.

```json
{
  "allowSudo": false,
  "description": "List user tables in a PostgreSQL database. Uses the host psql client, or a Docker postgres container when psql is absent.",
  "elevate": false,
  "id": "postgres-tables",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "database": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "database"
    ],
    "type": "object"
  },
  "name": "PostgreSQL tables",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `postgres-query`

Run one SQL statement using the remote psql authentication context, or inside a Docker postgres container.

```json
{
  "allowSudo": false,
  "description": "Run one SQL statement using the remote psql authentication context, or inside a Docker postgres container.",
  "elevate": false,
  "id": "postgres-query",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "database": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "query": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "database",
      "query"
    ],
    "type": "object"
  },
  "name": "PostgreSQL query",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `mysql-databases`

List databases visible to the configured MySQL client. Uses the host mysql client, or a Docker mysql/mariadb container when it is absent.

```json
{
  "allowSudo": false,
  "description": "List databases visible to the configured MySQL client. Uses the host mysql client, or a Docker mysql/mariadb container when it is absent.",
  "elevate": false,
  "id": "mysql-databases",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [],
    "type": "object"
  },
  "name": "MySQL databases",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `mysql-tables`

List tables in a MySQL database. Uses the host mysql client, or a Docker mysql/mariadb container when it is absent.

```json
{
  "allowSudo": false,
  "description": "List tables in a MySQL database. Uses the host mysql client, or a Docker mysql/mariadb container when it is absent.",
  "elevate": false,
  "id": "mysql-tables",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "database": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "database"
    ],
    "type": "object"
  },
  "name": "MySQL tables",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `mysql-query`

Run one SQL statement using the remote mysql authentication context, or inside a Docker mysql/mariadb container.

```json
{
  "allowSudo": false,
  "description": "Run one SQL statement using the remote mysql authentication context, or inside a Docker mysql/mariadb container.",
  "elevate": false,
  "id": "mysql-query",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "database": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "query": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "database",
      "query"
    ],
    "type": "object"
  },
  "name": "MySQL query",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `sqlite-files`

Find common SQLite database files below the remote login directory.

```json
{
  "allowSudo": false,
  "description": "Find common SQLite database files below the remote login directory.",
  "elevate": false,
  "id": "sqlite-files",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  "name": "SQLite files",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": false
}
```

## `sqlite-query`

Open a SQLite file and run one SQL statement.

```json
{
  "allowSudo": false,
  "description": "Open a SQLite file and run one SQL statement.",
  "elevate": false,
  "id": "sqlite-query",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "path": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "query": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "path",
      "query"
    ],
    "type": "object"
  },
  "name": "SQLite query",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `postgres-query-csv`

Run one SQL statement and return CSV rows with a header line.

```json
{
  "allowSudo": false,
  "description": "Run one SQL statement and return CSV rows with a header line.",
  "elevate": false,
  "id": "postgres-query-csv",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "database": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "query": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "database",
      "query"
    ],
    "type": "object"
  },
  "name": "PostgreSQL query (CSV)",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `mysql-query-batch`

Run one SQL statement and return tab-separated rows with a header line.

```json
{
  "allowSudo": false,
  "description": "Run one SQL statement and return tab-separated rows with a header line.",
  "elevate": false,
  "id": "mysql-query-batch",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "container": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "database": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "query": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "user": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "database",
      "query"
    ],
    "type": "object"
  },
  "name": "MySQL query (TSV)",
  "output": {
    "columns": [],
    "delimiter": "\t",
    "kind": "text"
  },
  "requiresConfirmation": true
}
```

## `sqlite-query-csv`

Open a SQLite file, run one SQL statement, and return CSV rows with a header line.

```json
{
  "allowSudo": false,
  "description": "Open a SQLite file, run one SQL statement, and return CSV rows with a header line.",
  "elevate": false,
  "id": "sqlite-query-csv",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {
      "path": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      },
      "query": {
        "description": "",
        "maxLength": 1024,
        "pattern": "^[^\\r\\n\\u0000]*$",
        "type": "string"
      }
    },
    "required": [
      "path",
      "query"
    ],
    "type": "object"
  },
  "name": "SQLite query (CSV)",
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
