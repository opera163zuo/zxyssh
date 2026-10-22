import { create } from 'zustand';
import { safeInvoke } from '../lib/tauri';

export interface PairedAgent {
  id: string;
  name: string;
  revoked: boolean;
}

export type PairingRequestStatus =
  | 'awaiting_approval'
  | 'approved'
  | 'rejected'
  | 'delivered'
  | 'expired';

export interface PairingRequest {
  id: string;
  agentName: string;
  peer: string;
  createdAt: number;
  expiresAt: number;
  status: PairingRequestStatus;
}

export interface PairingCode {
  code: string;
  expiresAt: number;
}

export interface LanListenerStatus {
  bind: string;
  allowLan: boolean;
  credentialSealing: boolean;
  approvalRequired: boolean;
  pid: number;
  startedAt: number;
  activeAgents: number;
  pendingRequests: number;
}

interface PairingState {
  code: PairingCode | null;
  requests: PairingRequest[];
  agents: PairedAgent[];
  approvalRequired: boolean;
  lanStatus: LanListenerStatus | null;
  /** A replacement credential is shown once, then dropped from memory. */
  issuedToken: string | null;
  busy: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  createCode: () => Promise<void>;
  approve: (requestId: string) => Promise<void>;
  reject: (requestId: string) => Promise<void>;
  revoke: (agentId: string) => Promise<void>;
  rename: (agentId: string, name: string) => Promise<void>;
  rotate: (agentId: string) => Promise<void>;
  dismissIssuedToken: () => void;
  setApprovalRequired: (required: boolean) => Promise<void>;
}

export const useMcpPairingStore = create<PairingState>((set, get) => {
  // Reading the registry is deliberately not gated on `busy`: every action
  // re-reads the lists while it still holds the flag, and gating the reload
  // would leave the panel showing a queue the operator just changed.
  const load = async () => {
    const [requests, agents, approval, lan] = await Promise.all([
      safeInvoke<PairingRequest[]>('mcp_pair_requests_list'),
      safeInvoke<PairedAgent[]>('mcp_agents_list'),
      safeInvoke<boolean>('mcp_pairing_approval_get'),
      safeInvoke<LanListenerStatus | null>('mcp_lan_status'),
    ]);
    for (const result of [requests, agents, approval]) {
      if (!result.success) {
        set({ error: result.error.message });
        return false;
      }
    }
    set({
      requests: requests.success ? requests.data : get().requests,
      agents: agents.success ? agents.data : get().agents,
      approvalRequired: approval.success ? approval.data : get().approvalRequired,
      // A missing listener status file means "not running", not an error.
      lanStatus: lan.success ? lan.data : null,
      error: null,
    });
    return true;
  };

  /** One registry write, then a reload. Concurrent calls are refused. */
  const write = async (command: string, args?: Record<string, unknown>) => {
    if (get().busy) return false;
    set({ busy: true });
    try {
      const result = await safeInvoke<unknown>(command, args);
      if (!result.success) {
        set({ error: result.error.message });
        return false;
      }
      await load();
      return true;
    } finally {
      set({ busy: false });
    }
  };

  return {
    code: null,
    requests: [],
    agents: [],
    approvalRequired: true,
    lanStatus: null,
    issuedToken: null,
    busy: false,
    error: null,

    refresh: async () => {
      if (get().busy) return;
      set({ busy: true });
      try {
        await load();
      } finally {
        set({ busy: false });
      }
    },

    createCode: async () => {
      if (get().busy) return;
      set({ busy: true });
      try {
        const result = await safeInvoke<PairingCode>('mcp_pair_code_create');
        if (!result.success) {
          set({ error: result.error.message });
          return;
        }
        set({ code: result.data, error: null });
      } finally {
        set({ busy: false });
      }
    },

    approve: async (requestId) => {
      await write('mcp_pair_request_approve', { requestId });
    },

    reject: async (requestId) => {
      await write('mcp_pair_request_reject', { requestId });
    },

    revoke: async (agentId) => {
      await write('mcp_agent_revoke', { agentId });
    },

    rename: async (agentId, name) => {
      await write('mcp_agent_rename', { agentId, name });
    },

    setApprovalRequired: async (required) => {
      await write('mcp_pairing_approval_set', { required });
    },

    rotate: async (agentId) => {
      if (get().busy) return;
      set({ busy: true });
      try {
        const result = await safeInvoke<{ id: string; name: string; token: string } | null>(
          'mcp_agent_rotate',
          { agentId },
        );
        if (!result.success) {
          set({ error: result.error.message });
          return;
        }
        set({ issuedToken: result.data?.token ?? null, error: null });
        await load();
      } finally {
        set({ busy: false });
      }
    },

    dismissIssuedToken: () => set({ issuedToken: null }),
  };
});
