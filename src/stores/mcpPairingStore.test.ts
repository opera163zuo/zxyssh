import { beforeEach, expect, it, vi } from 'vitest';

const safeInvoke = vi.fn();
vi.mock('../lib/tauri', () => {
  return {
    safeInvoke: (...args: unknown[]) => safeInvoke(...args),
  };
});

import { useMcpPairingStore } from './mcpPairingStore';

function ok(data: unknown) {
  return Promise.resolve({ success: true as const, data });
}
function fail(message: string) {
  return Promise.resolve({ success: false as const, error: { message } as never });
}

const listener = {
  bind: '192.168.0.96:8765',
  allowLan: true,
  credentialSealing: true,
  pid: 1,
  startedAt: 0,
  activeAgents: 1,
};

/**
 * Answer every read `load()` performs, so a test only overrides what it cares
 * about. An unlisted command is a mistake: `load()` has a fixed shape.
 */
function route(overrides: Record<string, () => Promise<unknown>> = {}) {
  safeInvoke.mockImplementation((command: string) => {
    const custom = overrides[command];
    if (custom) return custom();
    if (command === 'mcp_agents_list') return ok([]);
    if (command === 'mcp_pair_requests_list') return ok([]);
    if (command === 'mcp_lan_status') return ok(listener);
    throw new Error(`unexpected Tauri command: ${command}`);
  });
}

const calls = () => safeInvoke.mock.calls.map((call) => call[0]);

beforeEach(() => {
  safeInvoke.mockReset();
  useMcpPairingStore.setState({
    code: null,
    agents: [],
    requests: [],
    lanStatus: null,
    issuedToken: null,
    pairedJustNow: null,
    busy: false,
    error: null,
  });
});

it('reads the Agents, the waiting list and the listener in one refresh', async () => {
  route({
    mcp_agents_list: () => ok([{ id: 'a1', name: 'hermes-office', revoked: false }]),
    mcp_pair_requests_list: () =>
      ok([
        {
          id: 'r1',
          agentName: 'opencode',
          peer: '192.168.0.7:51234',
          createdAt: 0,
          expiresAt: 900,
          clientHasKey: true,
        },
      ]),
  });

  await useMcpPairingStore.getState().refresh();
  const state = useMcpPairingStore.getState();
  expect(state.agents[0].name).toBe('hermes-office');
  expect(state.requests[0].agentName).toBe('opencode');
  expect(state.lanStatus?.bind).toBe('192.168.0.96:8765');
  expect(state.error).toBeNull();
});

it('treats a missing listener file as stopped, not as an error', async () => {
  route({ mcp_lan_status: () => fail('no status file') });

  await useMcpPairingStore.getState().refresh();
  const state = useMcpPairingStore.getState();
  expect(state.lanStatus).toBeNull();
  expect(state.error).toBeNull();
});

it('surfaces a registry failure instead of pretending the list is empty', async () => {
  route({
    mcp_agents_list: () => fail('Agent registry is unreadable and was not modified: x'),
  });

  await useMcpPairingStore.getState().refresh();
  const state = useMcpPairingStore.getState();
  expect(state.error).toContain('unreadable');
  expect(state.agents).toEqual([]);
});

it('re-reads the registry after a revocation and passes the id through', async () => {
  route({ mcp_agent_revoke: () => ok(true) });

  await useMcpPairingStore.getState().revoke('a1');
  expect(useMcpPairingStore.getState().error).toBeNull();
  expect(calls()).toEqual([
    'mcp_agent_revoke',
    'mcp_agents_list',
    'mcp_pair_requests_list',
    'mcp_lan_status',
  ]);
  expect(safeInvoke.mock.calls[0][1]).toEqual({ agentId: 'a1' });
});

it('forgets a revoked record and reloads the list', async () => {
  route({ mcp_agent_forget: () => ok(true) });

  await useMcpPairingStore.getState().forget('bbbb2222');
  expect(calls()).toEqual([
    'mcp_agent_forget',
    'mcp_agents_list',
    'mcp_pair_requests_list',
    'mcp_lan_status',
  ]);
  expect(safeInvoke.mock.calls[0][1]).toEqual({ agentId: 'bbbb2222' });
});

it('approving a request reloads so the new Agent appears at once', async () => {
  route({
    mcp_pair_request_approve: () =>
      ok({ agent: { id: 'a2', name: 'opencode', revoked: false }, token: null }),
  });

  await useMcpPairingStore.getState().approveRequest('r1');
  expect(calls()).toEqual([
    'mcp_pair_request_approve',
    'mcp_agents_list',
    'mcp_pair_requests_list',
    'mcp_lan_status',
  ]);
  expect(safeInvoke.mock.calls[0][1]).toEqual({ requestId: 'r1' });
});

it('keeps an HTTP-only consent token in the one-time local banner', async () => {
  route({
    mcp_pair_request_approve: () =>
      ok({ agent: { id: 'a3', name: 'hermes', revoked: false }, token: 'vsh_once' }),
  });

  await useMcpPairingStore.getState().approveRequest('r-http');
  expect(useMcpPairingStore.getState().issuedToken).toBe('vsh_once');
  expect(calls()).toEqual([
    'mcp_pair_request_approve',
    'mcp_agents_list',
    'mcp_pair_requests_list',
    'mcp_lan_status',
  ]);
});

it('rejecting a request reloads the waiting list', async () => {
  route({ mcp_pair_request_reject: () => ok(true) });

  await useMcpPairingStore.getState().rejectRequest('r1');
  expect(calls()).toEqual([
    'mcp_pair_request_reject',
    'mcp_agents_list',
    'mcp_pair_requests_list',
    'mcp_lan_status',
  ]);
});

it('polls only the waiting list in the background', async () => {
  route({
    mcp_pair_requests_list: () =>
      ok([
        {
          id: 'r9',
          agentName: 'stranger',
          peer: '192.168.0.30:1',
          createdAt: 0,
          expiresAt: 9,
          clientHasKey: false,
        },
      ]),
  });

  await useMcpPairingStore.getState().refreshRequests();
  expect(calls()).toEqual(['mcp_pair_requests_list']);
  expect(useMcpPairingStore.getState().requests[0].agentName).toBe('stranger');
});

it('leaves a code alone while the registry still has it armed', async () => {
  useMcpPairingStore.setState({ code: { code: '123456', expiresAt: 99 } });
  route({ mcp_pair_code_pending: () => ok(true) });

  await useMcpPairingStore.getState().checkCode();
  expect(useMcpPairingStore.getState().code?.code).toBe('123456');
  expect(calls()).toEqual(['mcp_pair_code_pending']);
});

it('drops a spent code and names the Agent that just paired', async () => {
  useMcpPairingStore.setState({
    code: { code: '123456', expiresAt: 99 },
    agents: [{ id: 'a1', name: 'hermes-office', revoked: false }],
  });
  route({
    mcp_pair_code_pending: () => ok(false),
    mcp_agents_list: () =>
      ok([
        { id: 'a1', name: 'hermes-office', revoked: false },
        { id: 'b2', name: 'opencode-lan', revoked: false },
      ]),
  });

  await useMcpPairingStore.getState().checkCode();
  const state = useMcpPairingStore.getState();
  expect(state.code).toBeNull();
  expect(state.pairedJustNow).toBe('opencode-lan');
});
