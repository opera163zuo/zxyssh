import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { useMcpPairingStore } from '../../stores/mcpPairingStore';
import { AgentPairingPanel } from './AgentPairingPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const defaults = {
  code: null,
  agents: [] as { id: string; name: string; revoked: boolean }[],
  requests: [] as {
    id: string;
    agentName: string;
    peer: string;
    createdAt: number;
    expiresAt: number;
  }[],
  lanStatus: null,
  issuedToken: null,
  pairedJustNow: null,
  busy: false,
  error: null,
};

beforeEach(() => {
  useMcpPairingStore.setState({
    ...defaults,
    refresh: vi.fn(async () => {}),
    refreshRequests: vi.fn(async () => {}),
    createCode: vi.fn(async () => {}),
    checkCode: vi.fn(async () => {}),
    approveRequest: vi.fn(async () => {}),
    rejectRequest: vi.fn(async () => {}),
    revoke: vi.fn(async () => {}),
    forget: vi.fn(async () => {}),
    rename: vi.fn(async () => {}),
    rotate: vi.fn(async () => {}),
    dismissIssuedToken: vi.fn(),
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
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

it('rerolls a live code on click and announces an Agent that just paired', () => {
  useMcpPairingStore.setState({
    ...defaults,
    code: { code: '481516', expiresAt: Math.floor(Date.now() / 1000) + 120 },
  });
  render(<AgentPairingPanel />);
  fireEvent.click(screen.getByText('481516'));
  expect(useMcpPairingStore.getState().createCode).toHaveBeenCalledOnce();

  cleanup();
  useMcpPairingStore.setState({ ...defaults, pairedJustNow: 'opencode-lan' });
  render(<AgentPairingPanel />);
  expect(screen.getByText('settings.pairingJustPaired')).toBeInTheDocument();
});

it('shows a codeless request with its address and offers approve or reject', () => {
  useMcpPairingStore.setState({
    ...defaults,
    requests: [
      {
        id: 'r1',
        agentName: 'opencode',
        peer: '192.168.0.7:51234',
        createdAt: 0,
        expiresAt: Math.floor(Date.now() / 1000) + 300,
      },
    ],
  });
  render(<AgentPairingPanel />);
  expect(screen.getByText('opencode')).toBeInTheDocument();
  expect(screen.getByText(/192\.168\.0\.7:51234/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'settings.pairingRequestApprove' }));
  expect(useMcpPairingStore.getState().approveRequest).toHaveBeenCalledWith('r1');
  fireEvent.click(screen.getByRole('button', { name: 'settings.pairingRequestReject' }));
  expect(useMcpPairingStore.getState().rejectRequest).toHaveBeenCalledWith('r1');
});

it('lets the operator rename and rotate a live Agent, and delete only a revoked one', () => {
  useMcpPairingStore.setState({
    agents: [
      { id: 'aaaa1111', name: 'hermes-office', revoked: false },
      { id: 'bbbb2222', name: 'hermes-old', revoked: true },
    ],
  });
  render(<AgentPairingPanel />);
  fireEvent.click(screen.getByRole('button', { name: 'settings.pairingRotate' }));
  expect(useMcpPairingStore.getState().rotate).toHaveBeenCalledWith('aaaa1111');
  fireEvent.click(screen.getByRole('button', { name: 'settings.pairingRename' }));
  fireEvent.change(screen.getByDisplayValue('hermes-office'), { target: { value: 'hermes-desk' } });
  fireEvent.submit(screen.getByDisplayValue('hermes-desk').closest('form') as HTMLElement);
  expect(useMcpPairingStore.getState().rename).toHaveBeenCalledWith('aaaa1111', 'hermes-desk');
  // Actions are icon-only now, so the accessible name is the tooltip text: a
  // live row offers no delete, and exactly one delete exists for the revoked row.
  expect(screen.getAllByRole('button', { name: 'settings.pairingRevoke' })).toHaveLength(1);
  expect(screen.getAllByRole('button', { name: 'settings.pairingForget' })).toHaveLength(1);
  fireEvent.click(screen.getByRole('button', { name: 'settings.pairingForget' }));
  expect(useMcpPairingStore.getState().forget).toHaveBeenCalledWith('bbbb2222');
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
      pid: 4242,
      startedAt: 1,
      activeAgents: 2,
    },
    error: 'Agent registry is unreadable and was not modified: boom',
  });
  render(<AgentPairingPanel />);
  expect(screen.getByText('192.168.0.96:8765')).toBeInTheDocument();
  expect(screen.getByText('settings.pairingSealed')).toBeInTheDocument();
  expect(screen.getByText(/Agent registry is unreadable/)).toBeInTheDocument();
});
