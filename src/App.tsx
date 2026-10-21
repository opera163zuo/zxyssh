import { useCallback, useState, useRef, useEffect, useMemo, lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import {
  FolderOpen,
  Settings as SettingsIcon,
  ArrowLeft,
  PanelRightClose,
  ExternalLink,
  X,
  Bot,
} from 'lucide-react';
import { Mosaic, MosaicWindow, type MosaicNode, type MosaicBranch } from 'react-mosaic-component2';
import 'react-mosaic-component2/react-mosaic-component.css';
import { cn } from './lib/utils';
import { safeInvoke } from './lib/tauri';
import { withDeadline } from './lib/deadline';
import { useSessionStore, type Session } from './stores/sessionStore';
import { useNavigationStore } from './stores/navigationStore';
import { useNotificationStore } from './stores/notificationStore';
import { useThemeSync } from './lib/useThemeSync';
import { listenForLocalFileRequests, openLocalFiles } from './lib/localFiles';
import {
  openDetachedWindow, useDetachedOwnership, detachTargetKey, canCloseWorkspaceSession,
  readDetachedLayout, WORKSPACE_QUITTING_EVENT, type TransferSnapshot,
} from './lib/detach';
import { replaceLeaf, type PanePlacement } from './lib/docking';
import { receiveWindowDrops } from './lib/nativeDock';
import { trackWindowGeometry, captureWindowGeometry } from './lib/windowGeometry';
import { restoreWorkspaceLayout, restoreWorkspaceWindows, saveWorkspaceLayout } from './lib/workspacePersistence';
import { UPDATE_CHECK_INTERVAL_MS, useUpdateStore } from './stores/updateStore';
import { SessionTabs } from './components/SessionTabs';
import { TitleBar } from './components/TitleBar';
import { SftpPanel, SftpPanelHandle } from './components/SftpPanel';
import { AddServerDialog } from './components/AddServerDialog';
import { EditServerDialog } from './components/EditServerDialog';
import { ConnectDialog } from './components/ConnectDialog';
import { SelectServerDialog } from './components/SelectServerDialog';
import { ConfirmDialog } from './components/ConfirmDialog';
import { Notifications } from './components/Notifications';
import { AgentActivityPanel, AgentActivityNotice } from './components/AgentActivityPanel';
import { AgentApprovalDialog } from './components/AgentApprovalDialog';
import { WorkspaceToolbar } from './components/WorkspaceToolbar';
import { FingerprintVerificationDialog, FingerprintManagerDialog } from './components/FingerprintDialog';
import { ServerSidebar } from './components/ServerSidebar/ServerSidebar';
import { useServerStore, type Server } from './stores/serverStore';
import { useRuntimeCapabilitiesStore } from './stores/runtimeCapabilitiesStore';
import { useFileWorkspaceStore } from './stores/fileWorkspaceStore';
import { PaneDropZone } from './components/PaneDropZone';
import {
  parsePaneId,
  filePaneId,
  sessionPaneId,
  SESSION_PANE_PREFIX,
} from './lib/paneIds';
import type { TerminalHandle } from './components/Terminal';
import {
  removePane,
  pruneLeaves,
  getLeaves,
  countLeaves,
} from './lib/mosaicTree';

const Settings = lazy(() => import('./components/Settings').then((mod) => ({ default: mod.Settings })));
const Terminal = lazy(() => import('./components/Terminal').then((mod) => ({ default: mod.Terminal })));
const FileWorkspace = lazy(() => import('./components/FileWorkspace').then((mod) => ({ default: mod.FileWorkspace })));

function activateWorkspacePane(id: string): void {
  const pane = parsePaneId(id);
  const files = useFileWorkspaceStore.getState();
  files.activateTab(pane.kind === 'file' ? pane.id : null);
  const sessionId = pane.kind === 'session' ? pane.id
    : files.tabs.find((tab) => tab.id === pane.id)?.sessionId;
  if (sessionId && useSessionStore.getState().sessions.some(session => session.id === sessionId)) {
    useSessionStore.getState().setActiveSession(sessionId);
  }
}

function App() {
  const { t } = useTranslation();
  const {
    sessions,
    activeSessionId,
    setActiveSession,
    killSession,
    killLocalShellSession,
    fetchSessions,
    syncRemoteSessions,
    createLocalShellSession,
  } = useSessionStore();
  const { currentView, goToMain, goToSettings } = useNavigationStore();
  const { warning: notifyWarning, error: notifyError } = useNotificationStore();
  const { checkForUpdates, markVersionNotified } = useUpdateStore();
  const servers = useServerStore((state) => state.servers);
  const fetchServers = useServerStore((state) => state.fetchServers);
  const fetchGroups = useServerStore((state) => state.fetchGroups);
  const runtimeCapabilities = useRuntimeCapabilitiesStore((state) => state.capabilities);
  const loadRuntimeCapabilities = useRuntimeCapabilitiesStore((state) => state.load);
  const fileTabs = useFileWorkspaceStore((state) => state.tabs);
  const activeFileTabId = useFileWorkspaceStore((state) => state.activeTabId);
  const activateFileTab = useFileWorkspaceStore((state) => state.activateTab);
  const closeFileTab = useFileWorkspaceStore((state) => state.closeTab);

  const [isAddServerOpen, setIsAddServerOpen] = useState(false);
  const [isConnectOpen, setIsConnectOpen] = useState(false);
  const [isEditServerOpen, setIsEditServerOpen] = useState(false);
  const [isSelectServerOpen, setIsSelectServerOpen] = useState(false);
  const [sessionLauncherTab, setSessionLauncherTab] = useState<'local' | 'ssh' | undefined>();
  const [isSftpOpen, setIsSftpOpen] = useState(false);
  const [isAgentActivityOpen, setIsAgentActivityOpen] = useState(false);
  const [serverToConnect, setServerToConnect] = useState<Server | null>(null);
  const [connectForceNew, setConnectForceNew] = useState(false);
  const [serverToEdit, setServerToEdit] = useState<Server | null>(null);
  const [sessionToClose, setSessionToClose] = useState<string | null>(null);
  const [mosaicTree, setMosaicTree] = useState<MosaicNode<string> | null>(null);
  const [workspaceReady, setWorkspaceReady] = useState(!('__TAURI_INTERNALS__' in window));
  const workspaceInitialized = useRef(!('__TAURI_INTERNALS__' in window));
  const detachedOwners = useDetachedOwnership((state) => state.owners);
  const treeRef = useRef(mosaicTree);
  treeRef.current = mosaicTree;
  const focusedPaneRef = useRef<string | null>(null);
  const dockReceiverRef = useRef<(snapshot: TransferSnapshot, placement: PanePlacement | null) => boolean>(() => false);

  // Per-pane terminal handles. Mosaic can render multiple panes at once, so a
  // single ref is insufficient; each pane registers/unregisters via callback ref.
  const terminalRefs = useRef<Map<string, TerminalHandle>>(new Map());
  const sftpPanelRef = useRef<SftpPanelHandle>(null);
  const sessionBootstrapRef = useRef<Promise<void> | null>(null);

  useThemeSync();

  useEffect(() => {
    if (!workspaceReady) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    // Restoring layout must finish before files received from Finder are added.
    void Promise.resolve(sessionBootstrapRef.current).then(() => {
      if (disposed) return;
      return listenForLocalFileRequests().then(unlisten => { if (disposed) unlisten(); else stop = unlisten; });
    }).catch(console.error);
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'o') {
        event.preventDefault(); if (!event.repeat) void openLocalFiles();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => { disposed = true; stop?.(); window.removeEventListener('keydown', onKey); };
  }, [workspaceReady]);

  useEffect(() => {
    void loadRuntimeCapabilities();
  }, [loadRuntimeCapabilities]);

  useEffect(() => {
    void Promise.all([fetchServers(), fetchGroups()]);
  }, [fetchServers, fetchGroups]);

  useEffect(() => {
    if (!runtimeCapabilities.desktopUpdater) return;

    let cancelled = false;

    const checkUpdates = async () => {
      const release = await checkForUpdates();
      if (!release || cancelled) return;

      const { lastNotifiedVersion } = useUpdateStore.getState();
      if (lastNotifiedVersion === release.version) return;

      notifyWarning(
        t('updates.availableTitle'),
        t('updates.availableMessage', { version: release.version }),
        12000
      );
      markVersionNotified(release.version);
    };

    checkUpdates();
    const intervalId = window.setInterval(checkUpdates, UPDATE_CHECK_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [checkForUpdates, markVersionNotified, notifyWarning, runtimeCapabilities.desktopUpdater, t]);

  useEffect(() => {
    if (!sessionBootstrapRef.current) {
      sessionBootstrapRef.current = (async () => {
        try {
          const capabilities = await loadRuntimeCapabilities();
          await fetchSessions();
          if ('__TAURI_INTERNALS__' in window) {
            await syncRemoteSessions();
            const restored = await restoreWorkspaceLayout(capabilities.localShell);
            if (restored.layout) {
              focusedPaneRef.current = restored.layout.focusedPane;
              treeRef.current = restored.layout.tree;
              setMosaicTree(restored.layout.tree);
            }
            setWorkspaceReady(true);
            await restoreWorkspaceWindows(restored.layout);
            if (restored.warnings.length) {
              useNotificationStore.getState().warning(
                t('workspaceLayout.restored'),
                t('workspaceLayout.restoreAttention', { names: restored.warnings.join(', ') })
              );
            }
          }
          if (capabilities.localShell && useSessionStore.getState().sessions.length === 0) {
            await createLocalShellSession(undefined, 80, 24);
          }
          workspaceInitialized.current = true;
        } catch (error) {
          notifyError(t('workspaceLayout.restoreFailed'), String(error));
        } finally { setWorkspaceReady(true); }
      })();
    }

    // Poll session state every 2s, but pause while the window is hidden to
    // avoid pointless IPC when the app is in the background. On regaining
    // visibility, sync immediately so stale UI refreshes without waiting.
    let intervalId: number | null = window.setInterval(() => {
      if (document.hidden || !workspaceInitialized.current) return;
      void syncRemoteSessions();
    }, 2000);

    const handleVisibilityChange = () => {
      if (document.hidden) {
        if (intervalId !== null) {
          window.clearInterval(intervalId);
          intervalId = null;
        }
      } else {
        if (workspaceInitialized.current) void syncRemoteSessions();
        if (intervalId === null) {
          intervalId = window.setInterval(() => {
            if (document.hidden || !workspaceInitialized.current) return;
            void syncRemoteSessions();
          }, 2000);
        }
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);

    return () => {
      if (intervalId !== null) {
        window.clearInterval(intervalId);
      }
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, [createLocalShellSession, fetchSessions, loadRuntimeCapabilities, syncRemoteSessions]);

  useEffect(() => {
    if (!('__TAURI_INTERNALS__' in window)) return;
    let disposed = false;
    let closing = false;
    const stops: (() => void)[] = [];
    const keep = (stop: () => void) => disposed ? stop() : stops.push(stop);
    const flush = () => {
      if (!workspaceInitialized.current) return;
      saveWorkspaceLayout(treeRef.current, focusedPaneRef.current);
    };
    const quit = async () => {
      if (closing) return;
      closing = true;
      try {
        try { flush(); } catch (error) {
          notifyError(t('workspaceLayout.saveFailed'), String(error));
          if (!window.confirm(t('workspaceLayout.quitWithoutLayout'))) return;
        }
        try {
          await withDeadline((async () => {
            const { getCurrentWindow, Window: NativeWindow } = await import('@tauri-apps/api/window');
            await captureWindowGeometry('main', getCurrentWindow());
            for (const entry of readDetachedLayout()) {
              const label = useDetachedOwnership.getState().owners[detachTargetKey(entry.target)];
              const native = label ? await NativeWindow.getByLabel(label) : null;
              if (native) await captureWindowGeometry(entry.geometryKey, native);
            }
          })(), 1500, 'Window geometry capture timed out');
        } catch (error) { console.warn('[Workspace] Keeping last saved window bounds:', error); }
        const { emit } = await import('@tauri-apps/api/event');
        await withDeadline(emit(WORKSPACE_QUITTING_EVENT, { quitting: true }), 1000, 'Quit notification timed out').catch(console.warn);
        const result = await safeInvoke('workspace_exit');
        if (!result.success) throw result.error;
      } catch (error) {
        const { emit } = await import('@tauri-apps/api/event');
        void emit(WORKSPACE_QUITTING_EVENT, { quitting: false }).catch(console.warn);
        useNotificationStore.getState().error(t('workspaceLayout.saveFailed'), String(error));
      } finally { closing = false; }
    };
    void receiveWindowDrops((snapshot, placement) => dockReceiverRef.current(snapshot, placement)).then(keep).catch(console.error);
    void trackWindowGeometry('main').then(keep).catch(console.error);
    void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().onCloseRequested((event) => {
      event.preventDefault(); void quit();
    })).then(keep).catch(console.error);
    void import('@tauri-apps/api/event').then(({ listen }) => listen('zxyssh://save-and-quit', () => { void quit(); })).then((stop) => {
      keep(stop);
      if (!disposed) return safeInvoke('workspace_save_handler_ready');
    }).catch(console.error);
    const flushOnHide = () => { try { flush(); } catch (error) { console.error('[Workspace] Save failed:', error); } };
    window.addEventListener('pagehide', flushOnHide);
    return () => { disposed = true; stops.forEach((stop) => stop()); window.removeEventListener('pagehide', flushOnHide); };
  }, [t]);

  useEffect(() => {
    if (!workspaceReady || !workspaceInitialized.current || !('__TAURI_INTERNALS__' in window)) return;
    const timer = setTimeout(() => {
      try { saveWorkspaceLayout(treeRef.current, focusedPaneRef.current); }
      catch (error) { notifyError(t('workspaceLayout.saveFailed'), String(error)); }
    }, 200);
    return () => clearTimeout(timer);
  }, [workspaceReady, mosaicTree, sessions, fileTabs, detachedOwners, activeSessionId, activeFileTabId, notifyError, t]);

  const closeInactiveSession = useCallback(async (session: Session) => {
    if (!canCloseWorkspaceSession(session.id)) return false;
    // killSession/killLocalShellSession remove the tab on success (and when the
    // backend reports the session already gone). On other failures the tab is
    // kept so the periodic sync can reconcile instead of the tab flickering
    // back after a forced removal.
    if (session.sessionType === 'local') {
      await killLocalShellSession(session.id);
    } else {
      await killSession(session.id);
    }
    return true;
  }, [killSession, killLocalShellSession]);

  useEffect(() => {
    const handleKeyDown = async (event: KeyboardEvent) => {
      if (event.key === 'F12') {
        event.preventDefault();
        await safeInvoke('open_devtools');
        return;
      }

      const isCtrl = event.ctrlKey || event.metaKey;
      if (isCtrl && event.key.toLowerCase() === 'w' && activeFileTabId) {
        event.preventDefault();
        const fileTab = fileTabs.find((tab) => tab.id === activeFileTabId);
        if (!fileTab?.dirty || window.confirm(t('fileWorkspace.discardChanges'))) {
          closeFileTab(activeFileTabId);
        }
        return;
      }

      const target = event.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
        return;
      }

      if (isCtrl) {
        switch (event.key.toLowerCase()) {
          case 'n':
            event.preventDefault();
            setIsAddServerOpen(true);
            break;
          case ',':
            event.preventDefault();
            goToSettings();
            break;
          case 'w':
            event.preventDefault();
            if (activeSessionId) {
              const activeSession = sessions.find((s) => s.id === activeSessionId);
              const hasUnsavedFiles = fileTabs.some(
                (tab) => tab.sessionId === activeSessionId && tab.dirty
              );
              if (activeSession?.state === 'connected' || activeSession?.state === 'connecting' || hasUnsavedFiles) {
                setSessionToClose(activeSessionId);
              } else if (activeSession) {
                void closeInactiveSession(activeSession);
              }
            }
            break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [activeFileTabId, activeSessionId, closeFileTab, closeInactiveSession, fileTabs, goToSettings, sessions, t]);

  const connectedServerIds = useMemo(() => new Set(
    sessions
      .filter((s) => s.state === 'connected' || s.state === 'connecting')
      .map((s) => s.serverId)
  ), [sessions]);

  const activeSession = useMemo(
    () => sessions.find((s) => s.id === activeSessionId),
    [sessions, activeSessionId]
  );
  const activeFileTab = useMemo(
    () => fileTabs.find((tab) => tab.id === activeFileTabId) ?? null,
    [activeFileTabId, fileTabs]
  );

  useEffect(() => {
    if (
      activeFileTab
      && activeSessionId !== activeFileTab.sessionId
      && sessions.some((session) => session.id === activeFileTab.sessionId)
    ) {
      setActiveSession(activeFileTab.sessionId);
    }
  }, [activeFileTab, activeSessionId, sessions, setActiveSession]);

  // Selecting a new page in a split replaces only the focused pane, never the entire layout.
  useEffect(() => {
    if (!workspaceReady) return;
    const selected = activeFileTabId ? filePaneId(activeFileTabId)
      : activeSessionId ? sessionPaneId(activeSessionId) : null;
    if (!selected || detachedOwners[selected]) return;
    setMosaicTree((current) => {
      const leaves = getLeaves(current);
      if (leaves.includes(selected)) { focusedPaneRef.current = selected; return current; }
      if (leaves.length > 1) {
        const target = focusedPaneRef.current && leaves.includes(focusedPaneRef.current) ? focusedPaneRef.current : leaves[0];
        focusedPaneRef.current = selected;
        return replaceLeaf(current, target, selected);
      }
      // Standalone file pages keep their previous terminal alive until explicitly split.
      return parsePaneId(selected).kind === 'session' ? selected : current;
    });
  }, [workspaceReady, activeSessionId, activeFileTabId, detachedOwners]);

  useEffect(() => {
    if (!workspaceReady) return;
    const valid = new Set(sessions.map((session) => sessionPaneId(session.id)));
    for (const tab of fileTabs) valid.add(filePaneId(tab.id));
    for (const id of Object.keys(detachedOwners)) valid.delete(id);
    const fallback = activeSessionId && valid.has(sessionPaneId(activeSessionId))
      ? sessionPaneId(activeSessionId) : [...valid][0] ?? null;
    setMosaicTree((current) => pruneLeaves(current, valid) ?? fallback);
    const selected = activeFileTabId ? filePaneId(activeFileTabId)
      : activeSessionId ? sessionPaneId(activeSessionId) : null;
    if (selected && detachedOwners[selected]) {
      const next = getLeaves(treeRef.current).find((id) => valid.has(id)) ?? fallback;
      if (next) activateWorkspacePane(next);
      else { activateFileTab(null); setActiveSession(null); }
    }
  }, [workspaceReady, sessions, fileTabs, detachedOwners, activeSessionId, activeFileTabId, activateFileTab, setActiveSession]);

  const handleConnected = useCallback((sessionId: string) => {
    activateFileTab(null);
    setActiveSession(sessionId);

    // The Terminal component attaches after its event listener is ready so
    // the initial prompt/MOTD can be replayed without losing early output.
    setTimeout(() => {
      terminalRefs.current.get(sessionId)?.focus();
    }, 100);
  }, [activateFileTab, setActiveSession]);

  const handleConnect = useCallback(async (server: Server, options?: { forceNew?: boolean }) => {
    const forceNew = options?.forceNew ?? false;
    const status = await safeInvoke<{ authType: string | null; hasCredential: boolean }>(
      'credential_status', { request: { serverName: server.name } }
    );
    if (status.success && status.data.hasCredential) {
      const session = await useSessionStore.getState().connectWithSavedCredentials(server.name, 80, 24, forceNew);
      if (session) handleConnected(session.id);
      return;
    }
    setServerToConnect(server);
    setConnectForceNew(forceNew);
    setIsConnectOpen(true);
  }, [handleConnected]);

  const resolveServerForSession = useCallback(async (session: Session) => {
    let server = servers.find((candidate) =>
      candidate.id === session.serverId || candidate.name === session.serverName
    );

    if (server) {
      return server;
    }

    await fetchServers();
    server = useServerStore.getState().servers.find((candidate) =>
      candidate.id === session.serverId || candidate.name === session.serverName
    );

    return server ?? null;
  }, [servers, fetchServers]);

  const handleReconnectSession = useCallback(async (session: Session) => {
    if (session.sessionType !== 'ssh') {
      return;
    }

    const server = await resolveServerForSession(session);
    if (!server) {
      notifyWarning('Server Not Found', `Could not find a saved server for ${session.serverName}.`);
      return;
    }

    if (!await closeInactiveSession(session)) return;
    await handleConnect(server, { forceNew: true });
  }, [resolveServerForSession, notifyWarning, closeInactiveSession, handleConnect]);

  const handleAddServer = useCallback(() => {
    setIsAddServerOpen(true);
  }, []);

  const handleEditServer = useCallback((server: Server) => {
    setServerToEdit(server);
    setIsEditServerOpen(true);
  }, []);

  const handleNewSession = useCallback(() => {
    setSessionLauncherTab(undefined);
    setIsSelectServerOpen(true);
  }, []);

  const handleRemoveTerminalPane = useCallback((paneId: string) => {
    setMosaicTree((current) => {
      if (countLeaves(current) <= 1) return current;

      const next = removePane(current, paneId);
      // Pick a new active session from the remaining session panes.
      const remaining = getLeaves(next);
      const pane = parsePaneId(paneId);
      if (pane.kind === 'session' && activeSessionId === pane.id) {
        const nextActivePane = remaining.find((leaf) => leaf.startsWith(SESSION_PANE_PREFIX));
        if (nextActivePane) setActiveSession(nextActivePane.slice(SESSION_PANE_PREFIX.length));
      }
      return next;
    });
  }, [activeSessionId, setActiveSession]);

  const handleCollapseTerminalPanes = useCallback(() => {
    if (activeSessionId) setMosaicTree(sessionPaneId(activeSessionId));
  }, [activeSessionId]);

  const handleNewSessionForServer = useCallback((server: Server) => {
    handleConnect(server, { forceNew: true });
  }, [handleConnect]);

  const handleOpenSessionInWindow = useCallback((session: Session) => {
    void openDetachedWindow({
      kind: 'terminal',
      sessionId: session.id,
      title: session.serverName,
    });
  }, []);

  const handleSaveLayout = useCallback(async () => {
    if (!workspaceReady) return;
    try {
      saveWorkspaceLayout(treeRef.current, focusedPaneRef.current);
      workspaceInitialized.current = true;
      if ('__TAURI_INTERNALS__' in window) await withDeadline(captureWindowGeometry('main'), 1500, 'Window geometry capture timed out');
      useNotificationStore.getState().success(t('workspaceLayout.saved'), t('workspaceLayout.savedDescription'));
    } catch (error) { notifyError(t('workspaceLayout.saveFailed'), String(error)); }
  }, [workspaceReady, notifyError, t]);

  const handleOpenSftp = useCallback(() => {
    if (!activeSession) {
      notifyWarning('No Active Session', 'Connect to a server first to use SFTP.');
      return;
    }

    if (activeSession.sessionType === 'ssh' && activeSession.state !== 'connected') {
      notifyWarning('Session Disconnected', 'Reconnect the server before opening SFTP.');
      return;
    }

    if (!isSftpOpen) {
      setIsAgentActivityOpen(false);
    }
    sftpPanelRef.current?.toggle();
  }, [activeSession, isSftpOpen, notifyWarning]);

  const handleOpenAgentActivity = useCallback(() => {
    setIsAgentActivityOpen((current) => {
      const next = !current;
      if (next && isSftpOpen) {
        sftpPanelRef.current?.toggle();
      }
      if (next) {
        }
      return next;
    });
  }, [isSftpOpen]);

  const handleSftpCollapsedChange = useCallback((collapsed: boolean) => {
    setIsSftpOpen(!collapsed);
  }, []);

  const handleGatewaySessionsChanged = useCallback(() => {
    void syncRemoteSessions();
  }, [syncRemoteSessions]);

  const handleData = useCallback((_data: string) => {
  }, []);

  const handleConfirmCloseSession = useCallback(async () => {
    if (!sessionToClose) return;

    const sessionId = sessionToClose;
    if (!canCloseWorkspaceSession(sessionId)) { setSessionToClose(null); return; }
    const session = sessions.find((s) => s.id === sessionId);
    setSessionToClose(null);

    // The store removes the tab on success (and when the backend reports the
    // session already gone); on other failures the tab stays so the sync poll
    // can reconcile instead of the tab being resurrected after force-removal.
    if (session?.sessionType === 'local') {
      await killLocalShellSession(sessionId);
    } else {
      await killSession(sessionId);
    }
  }, [sessionToClose, sessions, killSession, killLocalShellSession]);

  const handleCancelCloseSession = useCallback(() => {
    setSessionToClose(null);
  }, []);

  const isSettingsView = currentView === 'settings';
  const isOverlayView = currentView !== 'main';
  const sessionToCloseObj = sessionToClose ? sessions.find((s) => s.id === sessionToClose) : null;
  const sessionToCloseDirtyFileCount = sessionToClose
    ? fileTabs.filter((tab) => tab.sessionId === sessionToClose && tab.dirty).length
    : 0;
  const sessionToCloseMessage = sessionToCloseObj?.sessionType === 'local'
    ? t('session.closeLocalShellConfirm', { name: sessionToCloseObj?.serverName })
    : t('session.closeSessionConfirm', { name: sessionToCloseObj?.serverName });
  const canRemoveTerminalPane = countLeaves(mosaicTree) > 1;
  const filePaneLeaves = new Set(getLeaves(mosaicTree).filter((leaf) => parsePaneId(leaf).kind === 'file'));
  const terminalAreaHidden = (activeFileTab !== null && !filePaneLeaves.has(filePaneId(activeFileTab.id)) && !detachedOwners[filePaneId(activeFileTab.id)])
;

  return (
    <div className="app-shell h-screen flex flex-col bg-tokyo-bg">
      <TitleBar
        activeSessionName={activeSession?.serverName}
        activeSessionType={activeSession?.sessionType}
        activeSessionState={activeSession?.state}
      />
      <div
        className="app-shell absolute inset-x-0 bottom-0 top-9 z-10 flex flex-col bg-tokyo-bg"
        style={{ display: isOverlayView ? 'flex' : 'none' }}
      >
        <Notifications />
        <header className="h-11 flex items-center px-4 bg-tokyo-bg-dark border-b border-tokyo-bg-hl">
          <button
            className={cn(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg border border-transparent',
              'text-tokyo-fg hover:text-tokyo-fg hover:bg-tokyo-bg-hl',
              'hover:border-tokyo-selection transition-colors duration-150',
              'focus:outline-none focus:ring-1 focus:ring-tokyo-blue'
            )}
            onClick={goToMain}
          >
            <ArrowLeft className="w-4 h-4" />
            <span className="text-sm">{t('common.back')}</span>
          </button>
          <h1 className="ml-4 text-tokyo-fg font-semibold">
            {t('settings.title')}
          </h1>
        </header>
        <div className="flex-1 overflow-y-auto bg-tokyo-bg">
          {isSettingsView && (
            <Suspense fallback={<div className="h-full bg-tokyo-bg" />}>
              <Settings />
            </Suspense>
          )}
        </div>
      </div>

      <div
        className="h-full flex flex-col flex-1"
        style={{ visibility: isOverlayView ? 'hidden' : 'visible' }}
      >
        <Notifications />

        <div className="flex-1 flex overflow-hidden">
          {!runtimeCapabilities.isMobile && (
            <ServerSidebar
              connectedServerIds={connectedServerIds}
              onConnect={handleConnect}
              onAddServer={handleAddServer}
              onEditServer={handleEditServer}
            />
          )}
          <main className="main-workspace flex-1 flex flex-col min-w-0 relative overflow-x-hidden w-full max-w-full">
            <SessionTabs
              onNewSession={handleNewSession}
              onReconnectSession={handleReconnectSession}
              onOpenSessionInWindow={handleOpenSessionInWindow}
              onSaveLayout={() => { void handleSaveLayout(); }}
              rightActions={(
                <WorkspaceToolbar
                  label={t('common.more')}
                  anyPressed={isAgentActivityOpen}
                  items={[
                    { id: 'sftp', label: t('sidebar.sftp'), icon: <FolderOpen className="h-4 w-4" />, disabled: !activeSession, onSelect: handleOpenSftp },
                    ...(runtimeCapabilities.agentGateway ? [{
                      id: 'agent-activity', label: t('agentActivity.title'), icon: <Bot className="h-4 w-4" />,
                      pressed: isAgentActivityOpen, onSelect: handleOpenAgentActivity,
                    }] : []),
                    ...(runtimeCapabilities.localShell && mosaicTree !== null && countLeaves(mosaicTree) > 1 ? [{
                      id: 'close-splits', label: t('session.closeSplits'), icon: <PanelRightClose className="h-4 w-4" />, onSelect: handleCollapseTerminalPanes,
                    }] : []),
                    { id: 'settings', label: t('sidebar.settings'), icon: <SettingsIcon className="h-4 w-4" />, onSelect: goToSettings },
                  ]}
                />
              )}
            />

            {runtimeCapabilities.agentGateway && <AgentActivityNotice onOpen={() => setIsAgentActivityOpen(true)} />}
            <div className="relative flex min-h-0 flex-1">
              <div className="workspace-return-zone flex min-w-0 flex-1 flex-col" onMouseDownCapture={(event) => {
                const pane = (event.target as Element).closest<HTMLElement>('[data-pane-id]');
                if (pane?.dataset.paneId) {
                  focusedPaneRef.current = pane.dataset.paneId;
                  activateWorkspacePane(pane.dataset.paneId);
                }
              }}>
                {fileTabs.filter((tab) => !filePaneLeaves.has(filePaneId(tab.id)) && !detachedOwners[filePaneId(tab.id)]).map((tab) => (
                  <div key={tab.id} data-pane-id={filePaneId(tab.id)} className={cn('relative min-h-0 flex-1', activeFileTabId === tab.id ? 'block' : 'hidden')}>
                    <Suspense fallback={<div className="h-full bg-tokyo-bg" />}>
                      <FileWorkspace tab={tab} isActive={activeFileTabId === tab.id} />
                    </Suspense>
                  </div>
                ))}
                <div className={cn('min-h-0 flex-1 flex-col', terminalAreaHidden ? 'hidden' : 'flex')}>
                  {sessions.length > 0 || mosaicTree !== null ? (
                    <>
                    <div className="mosaic-container relative min-h-0 flex-1 p-2">
                      <Mosaic<string>
                        value={mosaicTree}
                        onChange={(node) => { treeRef.current = node; setMosaicTree(node); }}
                        zeroStateView={<div className="workspace-return-zone flex h-full items-center justify-center p-8 text-center text-sm text-tokyo-comment">{t(workspaceReady ? 'workspaceLayout.empty' : 'common.loading')}</div>}
                        renderTile={(id: string, path: MosaicBranch[]) => {
                          const pane = parsePaneId(id);
                          if (detachedOwners[id] || !workspaceReady) return <div className="h-full bg-tokyo-bg" />;
                          if (pane.kind === 'file') {
                            const tab = fileTabs.find((candidate) => candidate.id === pane.id);
                            if (!tab) return <div />;
                            return (
                              <MosaicWindow<string> path={path} title={`${tab.dirty ? '● ' : ''}${tab.name}`} draggable
                                toolbarControls={[
                                  <button key="detach" className="icon-button h-5 w-5" aria-label={t('workspaceLayout.detach')} title={t('workspaceLayout.detach')} onClick={() => { void openDetachedWindow({ kind: 'file', sessionId: tab.sessionId, path: tab.path, name: tab.name, size: tab.size }); }}><ExternalLink className="h-3 w-3" /></button>,
                                  ...(canRemoveTerminalPane ? [<button key="close" className="icon-button h-5 w-5" onClick={() => handleRemoveTerminalPane(id)} aria-label={t('session.removePane', { name: tab.name })}><X className="h-3 w-3" /></button>] : []),
                                ]}>
                                <PaneDropZone paneId={id}><Suspense fallback={<div className="h-full bg-tokyo-bg" />}><FileWorkspace tab={tab} isActive={activeFileTabId === tab.id} /></Suspense></PaneDropZone>
                              </MosaicWindow>
                            );
                          }
                          const session = sessions.find((s) => s.id === pane.id);
                          const serverName = session?.serverName ?? t('session.localShell');
                          return (
                            <MosaicWindow<string>
                              path={path}
                              title={serverName}
                              draggable
                              toolbarControls={canRemoveTerminalPane ? [
                                <button
                                  key="close"
                                  className="icon-button h-5 w-5"
                                  onClick={() => handleRemoveTerminalPane(id)}
                                  aria-label={t('session.removePane', { name: serverName })}
                                  title={t('session.removePane', { name: serverName })}
                                >
                                  <X className="h-3 w-3" />
                                </button>,
                              ] : []}
                              className={cn(activeSessionId && id === sessionPaneId(activeSessionId) && 'mosaic-window-active')}
                            >
                              <PaneDropZone paneId={id}>
                                <div
                                  className="relative h-full bg-tokyo-bg"
                                  onMouseDown={() => {
                                    if (pane.id !== activeSessionId) setActiveSession(pane.id);
                                  }}
                                >
                                  <Suspense fallback={<div className="h-full bg-tokyo-bg" />}>
                                    <Terminal
                                      ref={(handle: TerminalHandle | null) => {
                                        if (handle) {
                                          terminalRefs.current.set(pane.id, handle);
                                        } else {
                                          terminalRefs.current.delete(pane.id);
                                        }
                                      }}
                                      sessionId={pane.id}
                                      onData={handleData}
                                    />
                                  </Suspense>
                                </div>
                              </PaneDropZone>
                            </MosaicWindow>
                          );
                        }}
                      />
                    </div>
                    </>
                  ) : (
                    <div className="h-full bg-tokyo-bg" aria-label={t('session.localShell')} />
                  )}
                </div>
              </div>

              <SftpPanel
                ref={sftpPanelRef}
                sessionId={activeSession?.id}
                sessionType={activeSession?.sessionType}
                defaultCollapsed={true}
                dock="right"
                onCollapsedChange={handleSftpCollapsedChange}
              />
              {runtimeCapabilities.agentGateway && (
                <AgentActivityPanel
                  open={isAgentActivityOpen}
                  onClose={() => setIsAgentActivityOpen(false)}
                  onSessionsChanged={handleGatewaySessionsChanged}
                />
              )}
            </div>
          </main>
        </div>
      </div>

      <AddServerDialog
        isOpen={isAddServerOpen}
        onClose={() => setIsAddServerOpen(false)}
      />

      <EditServerDialog
        isOpen={isEditServerOpen}
        server={serverToEdit}
        onClose={() => {
          setIsEditServerOpen(false);
          setServerToEdit(null);
        }}
      />

      <ConnectDialog
        isOpen={isConnectOpen}
        server={serverToConnect}
        onClose={() => {
          setIsConnectOpen(false);
          setServerToConnect(null);
          setConnectForceNew(false);
        }}
        forceNew={connectForceNew}
        onConnected={handleConnected}
      />

      <SelectServerDialog
        isOpen={isSelectServerOpen}
        initialTab={sessionLauncherTab}
        onClose={() => setIsSelectServerOpen(false)}
        onSelectServer={handleConnect}
        onAddServer={handleAddServer}
        onEditServer={handleEditServer}
        onNewSession={handleNewSessionForServer}
        connectedServerIds={connectedServerIds}
      />

      <ConfirmDialog
        isOpen={sessionToClose !== null}
        title={sessionToCloseObj?.sessionType === 'local' ? t('session.closeLocalShell') : t('session.closeSession')}
        message={sessionToCloseDirtyFileCount > 0
          ? `${sessionToCloseMessage}\n\n${t('fileWorkspace.sessionUnsavedWarning', { count: sessionToCloseDirtyFileCount })}`
          : sessionToCloseMessage}
        confirmLabel={sessionToCloseObj?.sessionType === 'local' ? t('session.closeLocalShell') : t('session.closeSession')}
        cancelLabel={t('common.cancel')}
        variant="danger"
        onConfirm={handleConfirmCloseSession}
        onCancel={handleCancelCloseSession}
      />

      <FingerprintVerificationDialog />

      <FingerprintManagerDialog />

      {runtimeCapabilities.agentGateway && <AgentApprovalDialog />}
    </div>
  );
}

export default App;
