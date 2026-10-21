import type { ReactNode } from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App';

interface TestSession {
  id: string;
  serverId: string;
  serverName: string;
  state: 'connected';
  createdAt: number;
  sessionType: 'local';
  purpose: 'shell';
  cwd?: string;
}

const testState = vi.hoisted(() => ({
  sessions: [] as TestSession[],
  activeSessionId: null as string | null,
  compact: true,
  capabilities: {
    platform: 'macos',
    isMobile: false,
    windowControls: true,
    localShell: true,
    agentGateway: true,
    desktopUpdater: false,
    cliIpc: true,
    directoryTransfer: true,
    backgroundTunnels: true,
  },
  actions: {
    setActiveSession: vi.fn(),
    killSession: vi.fn(async () => true),
    killLocalShellSession: vi.fn(async () => true),
    removeSession: vi.fn(),
    connectWithCredentials: vi.fn(async () => null),
    fetchSessions: vi.fn(async () => undefined),
    syncRemoteSessions: vi.fn(async () => undefined),
    createLocalShellSession: vi.fn(async () => null),
    initializeSettings: vi.fn(async () => undefined),
    loadRuntimeCapabilities: vi.fn(),
    fetchServers: vi.fn(async () => undefined),
    fetchGroups: vi.fn(async () => undefined),
    checkForUpdates: vi.fn(async () => null),
    markVersionNotified: vi.fn(),
    goToMain: vi.fn(),
    goToSettings: vi.fn(),
    notifyWarning: vi.fn(),
    notifyError: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('react-mosaic-component2', () => {
  const renderNode = (node: unknown, renderTile: (id: string, path: []) => ReactNode): ReactNode => {
    if (typeof node === 'string') return renderTile(node, []);
    if (node && typeof node === 'object' && 'first' in node && 'second' in node) {
      return (
        <>
          {renderNode((node as { first: unknown }).first, renderTile)}
          {renderNode((node as { second: unknown }).second, renderTile)}
        </>
      );
    }
    return null;
  };
  return {
    Mosaic: ({
      value,
      renderTile,
    }: {
      value: unknown;
      renderTile: (id: string, path: []) => ReactNode;
    }) => (
      <div
        data-testid="mosaic"
        data-mosaic-value={typeof value === 'string' ? value : 'branch'}
      >
        {renderNode(value, renderTile)}
      </div>
    ),
    MosaicWindow: ({
      title,
      toolbarControls = [],
      children,
    }: {
      title: string;
      toolbarControls?: ReactNode[];
      children: ReactNode;
    }) => (
      <section aria-label={`pane:${title}`}>
        <div data-testid="pane-toolbar">{toolbarControls}</div>
        {children}
      </section>
    ),
  };
});

vi.mock('./stores/sessionStore', () => {
  const getState = () => ({
    sessions: testState.sessions,
    activeSessionId: testState.activeSessionId,
    ...testState.actions,
  });
  const useSessionStore = (selector?: (state: ReturnType<typeof getState>) => unknown) => (
    selector ? selector(getState()) : getState()
  );
  return {
    useSessionStore: Object.assign(useSessionStore, { getState }),
  };
});

vi.mock('./stores/navigationStore', () => ({
  useNavigationStore: () => ({
    currentView: 'main',
    goToMain: testState.actions.goToMain,
    goToSettings: testState.actions.goToSettings,
  }),
}));

vi.mock('./stores/notificationStore', () => ({
  useNotificationStore: () => ({
    notifications: [],
    warning: testState.actions.notifyWarning,
    error: testState.actions.notifyError,
  }),
}));

vi.mock('./stores/settingsStore', () => {
  const getState = () => ({
    settings: {
      appearance: {
        theme: 'test-theme',
        themeMode: 'dark',
        lightTheme: 'paper-white',
        darkTheme: 'test-theme',
      },
    },
    initializeSettings: testState.actions.initializeSettings,
    updateAppearanceSettings: vi.fn(),
  });
  const useSettingsStore = (selector?: (state: ReturnType<typeof getState>) => unknown) =>
    selector ? selector(getState()) : getState();
  return {
    useSettingsStore: Object.assign(useSettingsStore, { getState }),
    themes: [{
      name: 'test-theme',
      colors: {
        bg: '#000000',
        bgDark: '#000000',
        bgHl: '#111111',
        fg: '#ffffff',
        fgDark: '#aaaaaa',
        accent: '#00aaff',
        onAccent: '#000000',
        red: '#ff0000',
        green: '#00ff00',
        yellow: '#ffff00',
        magenta: '#ff00ff',
        cyan: '#00ffff',
        orange: '#ff8800',
      },
    }],
  };
});

vi.mock('./stores/updateStore', () => {
  const useUpdateStore = Object.assign(
    () => ({
      checkForUpdates: testState.actions.checkForUpdates,
      markVersionNotified: testState.actions.markVersionNotified,
    }),
    { getState: () => ({ lastNotifiedVersion: null }) }
  );
  return { UPDATE_CHECK_INTERVAL_MS: 60_000, useUpdateStore };
});

vi.mock('./stores/serverStore', () => {
  const getState = () => ({
    servers: [],
    groups: [],
    fetchServers: testState.actions.fetchServers,
    fetchGroups: testState.actions.fetchGroups,
    deleteServer: vi.fn(),
    clearError: vi.fn(),
  });
  const useServerStore = (selector?: (state: ReturnType<typeof getState>) => unknown) => (
    selector ? selector(getState()) : getState()
  );
  return {
    useServerStore: Object.assign(useServerStore, { getState }),
    isKeyAuthType: (authType: string) => authType === 'key' || authType === 'key_with_passphrase',
  };
});

vi.mock('./stores/runtimeCapabilitiesStore', () => {
  const getState = () => ({
    capabilities: testState.capabilities,
    load: testState.actions.loadRuntimeCapabilities,
  });
  return {
    useRuntimeCapabilitiesStore: (
      selector: (state: ReturnType<typeof getState>) => unknown
    ) => selector(getState()),
  };
});

describe('App workspace actions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    testState.sessions = [{ id: 'session-1', serverId: 'shell-1', serverName: 'Local Shell', state: 'connected', createdAt: 1, sessionType: 'local', purpose: 'shell' }];
    testState.activeSessionId = 'session-1';
    testState.compact = true;
    testState.capabilities = {
      platform: 'macos',
      isMobile: false,
      windowControls: true,
      localShell: true,
      agentGateway: true,
      desktopUpdater: false,
      cliIpc: true,
      directoryTransfer: true,
      backgroundTunnels: true,
    };
    testState.actions.loadRuntimeCapabilities.mockImplementation(async () => (
      testState.capabilities
    ));
    vi.spyOn(console, 'log').mockImplementation(() => undefined);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('does not expose a remove control for the sole Mosaic pane', async () => {
    render(<App />);

    const mosaic = await screen.findByTestId('mosaic');
    expect(mosaic).toHaveAttribute('data-mosaic-value', 'session:session-1');

    const toolbar = await screen.findByTestId('pane-toolbar');
    expect(toolbar).toBeEmptyDOMElement();
    expect(
      within(toolbar).queryByRole('button', { name: 'session.removePane' })
    ).not.toBeInTheDocument();
  });

});
