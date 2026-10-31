import { create } from 'zustand';
import { safeInvoke } from '../lib/tauri';

export interface PairedAgent {
  id: string;
  name: string;
  revoked: boolean;
}

export interface PairingCode {
  code: string;
  expiresAt: number;
}

export interface LanListenerStatus {
  bind: string;
  allowLan: boolean;
  credentialSealing: boolean;
  pid: number;
  startedAt: number;
  activeAgents: number;
}

interface PairingState {
  code: PairingCode | null;
  agents: PairedAgent[];
  lanStatus: LanListenerStatus | null;
  /** A replacement credential is shown once, then dropped from memory. */
  issuedToken: string | null;
  /** Set when a code the panel was showing was spent by an Agent. */
  pairedJustNow: string | null;
  busy: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  createCode: () => Promise<void>;
  checkCode: () => Promise<void>;
  revoke: (agentId: string) => Promise<void>;
  rename: (agentId: string, name: string) => Promise<void>;
  rotate: (agentId: string) => Promise<void>;
  dismissIssuedToken: () => void;
}

export const useMcpPairingStore = create<PairingState>((set, get) => {
  const load = async () => {
    const [agents, lan] = await Promise.all([
      safeInvoke<PairedAgent[]>('mcp_agents_list'),
      safeInvoke<LanListenerStatus | null>('mcp_lan_status'),
    ]);
    if (!agents.success) {
      set({ error: agents.error.message });
      return false;
    }
    set({
      agents: agents.data,
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
    agents: [],
    lanStatus: null,
    issuedToken: null,
    pairedJustNow: null,
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
        set({ code: result.data, pairedJustNow: null, error: null });
      } finally {
        set({ busy: false });
      }
    },

    /**
     * Ask the registry whether the displayed code is still unspent. The
     * listener spends it in another process, so this is the only way the panel
     * learns a pairing just succeeded; on the first no it drops the code and
     * names the Agent that arrived.
     */
    checkCode: async () => {
      if (get().busy || get().code === null) return;
      const pending = await safeInvoke<boolean>('mcp_pair_code_pending');
      if (!pending.success) {
        set({ error: pending.error.message });
        return;
      }
      if (pending.data) return;
      const known = new Set(get().agents.map((agent) => agent.id));
      await load();
      const arrived = get().agents.find((agent) => !known.has(agent.id) && !agent.revoked);
      set({ code: null, pairedJustNow: arrived?.name ?? null });
    },

    revoke: async (agentId) => {
      await write('mcp_agent_revoke', { agentId });
    },

    rename: async (agentId, name) => {
      await write('mcp_agent_rename', { agentId, name });
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
