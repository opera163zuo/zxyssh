import { beforeEach, expect, it, vi } from 'vitest';

const safeInvoke = vi.fn();
vi.mock('../lib/tauri', () => ({
  safeInvoke: (...args: unknown[]) => safeInvoke(...args),
}));

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

beforeEach(() => {
  safeInvoke.mockReset();
  useMcpPairingStore.setState({
    code: null,
    agents: [],
    lanStatus: null,
    issuedToken: null,
    pairedJustNow: null,
    busy: false,
    error: null,
  });
});

it('reads the Agents and the listener in one refresh', async () => {
  safeInvoke
    .mockImplementationOnce((command) => {
      expect(command).toBe('mcp_agents_list');
      return ok([{ id: 'a1', name: 'hermes-office', revoked: false }]);
    })
    .mockImplementationOnce((command) => {
      expect(command).toBe('mcp_lan_status');
      return ok(listener);
    });

  await useMcpPairingStore.getState().refresh();
  const state = useMcpPairingStore.getState();
  expect(state.agents[0].name).toBe('hermes-office');
  expect(state.lanStatus?.bind).toBe('192.168.0.96:8765');
  expect(state.error).toBeNull();
});

it('treats a missing listener file as stopped, not as an error', async () => {
  safeInvoke.mockImplementationOnce(() => ok([])).mockImplementationOnce(() => fail('no status file'));

  await useMcpPairingStore.getState().refresh();
  const state = useMcpPairingStore.getState();
  expect(state.lanStatus).toBeNull();
  expect(state.error).toBeNull();
});

it('surfaces a registry failure instead of pretending the list is empty', async () => {
  safeInvoke.mockImplementationOnce(() => fail('Agent registry is unreadable and was not modified: x')).mockImplementationOnce(() => ok(null));

  await useMcpPairingStore.getState().refresh();
  const state = useMcpPairingStore.getState();
  expect(state.error).toContain('unreadable');
  expect(state.agents).toEqual([]);
});

it('re-reads the registry after a revocation and passes the id through', async () => {
  safeInvoke
    .mockImplementationOnce((command, args) => {
      expect(command).toBe('mcp_agent_revoke');
      expect(args).toEqual({ agentId: 'a1' });
      return ok(true);
    })
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok(null));

  await useMcpPairingStore.getState().revoke('a1');
  expect(useMcpPairingStore.getState().error).toBeNull();
  expect(safeInvoke.mock.calls.map((call) => call[0])).toEqual([
    'mcp_agent_revoke',
    'mcp_agents_list',
    'mcp_lan_status',
  ]);
});

it('forgets a revoked record and reloads the list', async () => {
  safeInvoke
    .mockImplementationOnce((command, args) => {
      expect(command).toBe('mcp_agent_forget');
      expect(args).toEqual({ agentId: 'bbbb2222' });
      return ok(true);
    })
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok(null));

  await useMcpPairingStore.getState().forget('bbbb2222');
  expect(safeInvoke.mock.calls.map((call) => call[0])).toEqual([
    'mcp_agent_forget',
    'mcp_agents_list',
    'mcp_lan_status',
  ]);
});

it('leaves a code alone while the registry still has it armed', async () => {
  useMcpPairingStore.setState({ code: { code: '123456', expiresAt: 99 } });
  safeInvoke.mockImplementationOnce((command) => {
    expect(command).toBe('mcp_pair_code_pending');
    return ok(true);
  });

  await useMcpPairingStore.getState().checkCode();
  expect(useMcpPairingStore.getState().code?.code).toBe('123456');
  expect(safeInvoke).toHaveBeenCalledTimes(1);
});

it('drops a spent code and names the Agent that just paired', async () => {
  useMcpPairingStore.setState({
    code: { code: '123456', expiresAt: 99 },
    agents: [{ id: 'a1', name: 'hermes-office', revoked: false }],
  });
  safeInvoke
    .mockImplementationOnce(() => ok(false))
    .mockImplementationOnce(() =>
      ok([
        { id: 'a1', name: 'hermes-office', revoked: false },
        { id: 'b2', name: 'opencode-lan', revoked: false },
      ]),
    )
    .mockImplementationOnce(() => ok(listener));

  await useMcpPairingStore.getState().checkCode();
  const state = useMcpPairingStore.getState();
  expect(state.code).toBeNull();
  expect(state.pairedJustNow).toBe('opencode-lan');
});
