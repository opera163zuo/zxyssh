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

beforeEach(() => {
  safeInvoke.mockReset();
  useMcpPairingStore.setState({
    code: null,
    requests: [],
    agents: [],
    approvalRequired: true,
    lanStatus: null,
    issuedToken: null,
    busy: false,
    error: null,
  });
});

it('reads the queue, the Agents and the listener in one refresh', async () => {
  safeInvoke
    .mockImplementationOnce((command) => {
      expect(command).toBe('mcp_pair_requests_list');
      return ok([{ id: 'r1', agentName: 'hermes', peer: '192.168.0.20', createdAt: 1, expiresAt: 2, status: 'awaiting_approval' }]);
    })
    .mockImplementationOnce((command) => {
      expect(command).toBe('mcp_agents_list');
      return ok([{ id: 'a1', name: 'hermes-office', revoked: false }]);
    })
    .mockImplementationOnce((command) => {
      expect(command).toBe('mcp_pairing_approval_get');
      return ok(true);
    })
    .mockImplementationOnce((command) => {
      expect(command).toBe('mcp_lan_status');
      return ok({ bind: '192.168.0.96:8765', allowLan: true, credentialSealing: true, approvalRequired: true, pid: 1, startedAt: 0, activeAgents: 1, pendingRequests: 1 });
    });

  await useMcpPairingStore.getState().refresh();
  const state = useMcpPairingStore.getState();
  expect(state.requests).toHaveLength(1);
  expect(state.agents[0].name).toBe('hermes-office');
  expect(state.lanStatus?.bind).toBe('192.168.0.96:8765');
  expect(state.error).toBeNull();
});

it('treats a missing listener file as stopped, not as an error', async () => {
  safeInvoke
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok(false))
    .mockImplementationOnce(() => fail('no status file'));

  await useMcpPairingStore.getState().refresh();
  expect(useMcpPairingStore.getState().lanStatus).toBeNull();
});

it('surfaces a registry failure instead of pretending the queue is empty', async () => {
  safeInvoke
    .mockImplementationOnce(() => fail('Agent registry is unreadable and was not modified: x'))
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok(true))
    .mockImplementationOnce(() => ok(null));

  await useMcpPairingStore.getState().refresh();
  expect(useMcpPairingStore.getState().error).toContain('unreadable');
  expect(useMcpPairingStore.getState().requests).toEqual([]);
});

it('re-reads the queue after an approval and passes the id through', async () => {
  safeInvoke
    .mockImplementationOnce((command, args) => {
      expect(command).toBe('mcp_pair_request_approve');
      expect(args).toEqual({ requestId: 'r1' });
      return ok({ id: 'a1', name: 'hermes', revoked: false });
    })
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok(true))
    .mockImplementationOnce(() => ok(null));

  await useMcpPairingStore.getState().approve('r1');
  expect(useMcpPairingStore.getState().error).toBeNull();
  // One approval write, then the queue is re-read.
  const commands = safeInvoke.mock.calls.map((call) => call[0]);
  expect(commands).toEqual([
    'mcp_pair_request_approve',
    'mcp_pair_requests_list',
    'mcp_agents_list',
    'mcp_pairing_approval_get',
    'mcp_lan_status',
  ]);
});

it('will not run two registry reads at once', async () => {
  const resolvers: Array<() => void> = [];
  safeInvoke.mockImplementation(
    () =>
      new Promise((resolve) => {
        resolvers.push(() => resolve({ success: true as const, data: [] }));
      }),
  );
  const first = useMcpPairingStore.getState().refresh();
  await Promise.resolve();
  expect(useMcpPairingStore.getState().busy).toBe(true);
  await useMcpPairingStore.getState().createCode();
  expect(safeInvoke).not.toHaveBeenCalledWith('mcp_pair_code_create');
  for (const release of resolvers) release();
  await first;
  expect(useMcpPairingStore.getState().busy).toBe(false);
});

it('keeps a rotated credential in memory only until it is dismissed', async () => {
  safeInvoke
    .mockImplementationOnce((command, args) => {
      expect(command).toBe('mcp_agent_rotate');
      expect(args).toEqual({ agentId: 'a1' });
      return ok({ id: 'a1', name: 'hermes', token: 'vsh_new' });
    })
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok([]))
    .mockImplementationOnce(() => ok(true))
    .mockImplementationOnce(() => ok(null));

  await useMcpPairingStore.getState().rotate('a1');
  expect(useMcpPairingStore.getState().issuedToken).toBe('vsh_new');
  useMcpPairingStore.getState().dismissIssuedToken();
  expect(useMcpPairingStore.getState().issuedToken).toBeNull();
});


