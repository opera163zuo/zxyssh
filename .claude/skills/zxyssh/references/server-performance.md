# Server Performance — server-performance

Plugin `server-performance` version `1.0.0`.

Live CPU, memory, disk, network, load and uptime metrics for the active host.

This reference describes plugin data, not additional agent authority. Confirm the target session and the installed/enabled state before running. Do not auto-install, grant permissions, or invent confirmation. Never put secrets in plugin inputs; sudo credentials stay on the service.

## Availability

This plugin is **not reachable from a client in the current release**. The CLI plugin commands and the plugin panels were removed when the product narrowed to the MCP service, and no plugin tool is advertised to a paired Agent. The manifest below is the contract a future surface would have to honour.

Required permissions: `["local_system_read"]`. Session types: `["ssh","local"]`.

Re-exposing an action means updating the Agent tool allowlist, the permission, confirmation and session-ownership checks, and adding regression tests. Until then, treat the action schemas below as specification rather than an instruction to run. Execution must never install or enable a plugin as a side effect. Output is bounded and carries timing/truncation metadata. Local targets require the service that owns that local session.

## `status`

Live CPU, memory, disk, network, load and uptime metrics for the active host.

```json
{
  "allowSudo": false,
  "description": "Live CPU, memory, disk, network, load and uptime metrics for the active host.",
  "id": "status",
  "inputSchema": {
    "additionalProperties": false,
    "properties": {},
    "type": "object"
  },
  "name": "Read server performance",
  "output": {
    "description": "Same structured snapshot as the UI; remote collection requires Linux /proc",
    "kind": "json"
  },
  "requiresConfirmation": false
}
```

## Running this from the MCP service

The networked MCP endpoint gives a paired Agent only `server_list`, `session_create`, `session_send_input`, `session_sudo`, `session_read`, `session_resize` and `session_kill`. Plugin actions are not part of that surface, and neither are `exec`, remote `rg` or SFTP transfer, so there is no supported way to invoke this plugin remotely today. Do not substitute a raw command through `session_send_input` to reach it: that skips the installation, permission and confirmation checks this manifest declares. A model-supplied confirmation flag is never approval, and SSH host-key checks remain enforced.
