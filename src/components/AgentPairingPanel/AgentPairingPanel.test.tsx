import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { useMcpPairingStore, type PairingRequest } from '../../stores/mcpPairingStore';
import { AgentPairingPanel } from './AgentPairingPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const defaults = {
  code: null,
  requests: [] as PairingRequest[],
  agents: [] as { id: string; name: string; revoked: boolean }[],
  approvalRequired: true,
  lanStatus: null,
  issuedToken: null,
  busy: false,
  error: null,
};

beforeEach(() => {
  useMcpPairingStore.setState({
    ...defaults,
    refresh: vi.fn(async () => {}),
    createCode: vi.fn(async () => {}),
    approve: vi.fn(async () => {}),
    reject: vi.fn(async () => {}),
    revoke: vi.fn(async () => {}),
    rename: vi.fn(async () => {}),
    rotate: vi.fn(async () => {}),
    dismissIssuedToken: vi.fn(),
    setApprovalRequired: vi.fn(async () => {}),
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it('shows an awaiting request with its peer and wires the decision buttons', () => {
  useMcpPairingStore.setState({
    requests: [
      {
        id: 'req-1',
        agentName: 'hermes-laptop',
        peer: '192.168.0.20',
        createdAt: 1,
        expiresAt: Math.floor(Date.now() / 1000) + 300,
        status: 'awaiting_approval',
      },
    ],
  });
  render(<AgentPairingPanel />);
  expect(screen.getByText('hermes-laptop')).toBeInTheDocument();
  expect(screen.getByText('192.168.0.20')).toBeInTheDocument();
  fireEvent.click(screen.getByText('settings.pairingApprove'));
  expect(useMcpPairingStore.getState().approve).toHaveBeenCalledWith('req-1');
  fireEvent.click(screen.getByText('settings.pairingReject'));
  expect(useMcpPairingStore.getState().reject).toHaveBeenCalledWith('req-1');
});

it('creates a pairing code and then shows it with its countdown', async () => {
  render(<AgentPairingPanel />);
  fireEvent.click(screen.getByText('settings.pairingCodeCreate'));
  expect(useMcpPairingStore.getState().createCode).toHaveBeenCalledOnce();

  cleanup();
  useMcpPairingStore.setState({
    ...defaults,
    code: { code: '481516', expiresAt: Math.floor(Date.now() / 1000) + 120 },
  });
  render(<AgentPairingPanel />);
  expect(screen.getByText('481516')).toBeInTheDocument();
  expect(screen.queryByText('settings.pairingCodeCreate')).not.toBeInTheDocument();
});

it('lets the operator rename, rotate and revoke a live Agent but not a revoked one', () => {
  useMcpPairingStore.setState({
    agents: [
      { id: 'aaaa1111', name: 'hermes-office', revoked: false },
      { id: 'bbbb2222', name: 'hermes-old', revoked: true },
    ],
  });
  render(<AgentPairingPanel />);
  expect(screen.getByText('settings.pairingRevoke')).toBeInTheDocument();
  fireEvent.click(screen.getByText('settings.pairingRotate'));
  expect(useMcpPairingStore.getState().rotate).toHaveBeenCalledWith('aaaa1111');
  fireEvent.click(screen.getByText('settings.pairingRename'));
  fireEvent.change(screen.getByDisplayValue('hermes-office'), { target: { value: 'hermes-desk' } });
  fireEvent.submit(screen.getByDisplayValue('hermes-desk').closest('form') as HTMLElement);
  expect(useMcpPairingStore.getState().rename).toHaveBeenCalledWith('aaaa1111', 'hermes-desk');
  // The revoked row offers no actions at all.
  expect(screen.getAllByText('settings.pairingRevoke')).toHaveLength(1);
});

it('shows a rotated credential until it is dismissed', () => {
  useMcpPairingStore.setState({ issuedToken: 'vsh_visible-once' });
  render(<AgentPairingPanel />);
  expect(screen.getByText('vsh_visible-once')).toBeInTheDocument();
  fireEvent.click(screen.getByText('settings.pairingTokenDone'));
  expect(useMcpPairingStore.getState().dismissIssuedToken).toHaveBeenCalledOnce();
});

it('reports the listener state and surfaces a store error', () => {
  useMcpPairingStore.setState({
    lanStatus: {
      bind: '192.168.0.96:8765',
      allowLan: true,
      credentialSealing: true,
      approvalRequired: true,
      pid: 4242,
      startedAt: 1,
      activeAgents: 2,
      pendingRequests: 1,
    },
    error: 'Agent registry is unreadable and was not modified: boom',
  });
  render(<AgentPairingPanel />);
  expect(screen.getByText('192.168.0.96:8765')).toBeInTheDocument();
  expect(screen.getByText('settings.pairingSealed')).toBeInTheDocument();
  expect(screen.getByText(/Agent registry is unreadable/)).toBeInTheDocument();
});
