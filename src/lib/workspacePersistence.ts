import type { MosaicNode } from 'react-mosaic-component2';
import { useSessionStore, type Session } from '../stores/sessionStore';
import { useFileWorkspaceStore, type FileWorkspaceTab } from '../stores/fileWorkspaceStore';
import { parsePaneId, sessionPaneId, filePaneId } from './paneIds';
import { sanitizeTree } from './docking';
import { readDetachedLayout, saveDetachedLayout, restoreDetachedWindows, type DetachedLayoutEntry } from './detach';
import { readTextBuffer, writeTextBuffer } from './fileEditBuffer';

const KEY = 'zxyssh.workspace-layout.v2';
export type SavedSession = Pick<Session, 'id' | 'serverId' | 'serverName' | 'sessionType' | 'purpose' | 'cwd'>;
export interface WorkspaceLayout {
  version: 2;
  tree: MosaicNode<string> | null;
  focusedPane: string | null;
  sessions: SavedSession[];
  files: FileWorkspaceTab[];
  activeSessionId: string | null;
  activeFileId: string | null;
  detached: DetachedLayoutEntry[];
}
const string = (value: unknown): value is string => typeof value === 'string' && value.length > 0 && value.length < 8192;

export function readWorkspaceLayout(): WorkspaceLayout | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? 'null') as WorkspaceLayout | null;
    if (!value || value.version !== 2 || !Array.isArray(value.sessions)
      || !Array.isArray(value.files) || !Array.isArray(value.detached)) return null;
    return {
      ...value,
      tree: sanitizeTree(value.tree),
      focusedPane: string(value.focusedPane) ? value.focusedPane : null,
      sessions: value.sessions.slice(0, 64).filter((s) => s && string(s.id) && string(s.serverName)
        && string(s.serverId) && (s.sessionType === 'ssh' || s.sessionType === 'local')),
      files: value.files.slice(0, 128).filter((f) => f && string(f.id) && string(f.sessionId) && string(f.path) && string(f.name) && Number.isFinite(f.size)),
      detached: value.detached.slice(0, 32).filter((d) => d?.target && string(d.target.sessionId)
        && ['terminal', 'file'].includes(d.target.kind) && string(d.geometryKey)),
    };
  } catch { return null; }
}

export function saveWorkspaceLayout(tree: MosaicNode<string> | null, focusedPane: string | null): void {
  const sessions = useSessionStore.getState();
  const files = useFileWorkspaceStore.getState();
  const layout: WorkspaceLayout = {
    version: 2, tree: sanitizeTree(tree), focusedPane,
    // Explicit allowlist: no authentication fields, commands, plugin inputs or output.
    sessions: sessions.sessions.map(({ id, serverId, serverName, sessionType, purpose, cwd }) => ({ id, serverId, serverName, sessionType, purpose, cwd })),
    files: files.tabs, activeSessionId: sessions.activeSessionId, activeFileId: files.activeTabId,
    detached: readDetachedLayout(),
  };
  localStorage.setItem(KEY, JSON.stringify(layout));
}

/** A runtime session ID is not a persistent identity. Re-map every reference together. */
export function remapWorkspace(layout: WorkspaceLayout, sessions: Map<string, Session>): WorkspaceLayout {
  const files = layout.files.flatMap((tab) => {
    if (tab.source === 'local') return [tab];
    const session = sessions.get(tab.sessionId);
    return session ? [{ ...tab, sessionId: session.id, id: `${session.id}\u0000${tab.path}` }] : [];
  });
  const ids = new Map<string, string>();
  for (const [previous, session] of sessions) ids.set(sessionPaneId(previous), sessionPaneId(session.id));
  for (const tab of layout.files) {
    if (tab.source === 'local') { ids.set(filePaneId(tab.id), filePaneId(tab.id)); continue; }
    const session = sessions.get(tab.sessionId);
    if (session) ids.set(filePaneId(tab.id), filePaneId(`${session.id}\u0000${tab.path}`));
  }
  const visit = (node: MosaicNode<string> | null): MosaicNode<string> | null => {
    if (node === null) return null;
    if (typeof node === 'string') return ids.get(node) ?? null;
    const first = visit(node.first); const second = visit(node.second);
    return first && second ? { ...node, first, second } : first ?? second;
  };
  const detached = layout.detached.flatMap((entry): DetachedLayoutEntry[] => {
    if (entry.target.kind === 'file' && entry.target.source === 'local') return [entry];
    const session = sessions.get(entry.target.sessionId);
    if (!session) return [];
    const target = { ...entry.target, sessionId: session.id };
    return [{ target, geometryKey: entry.geometryKey }];
  });
  const activeFilePane = layout.activeFileId ? ids.get(filePaneId(layout.activeFileId)) : null;
  return {
    ...layout, tree: sanitizeTree(visit(layout.tree)), files, detached,
    sessions: [...sessions.values()],
    focusedPane: layout.focusedPane ? ids.get(layout.focusedPane) ?? null : null,
    activeSessionId: layout.activeSessionId ? sessions.get(layout.activeSessionId)?.id ?? null : null,
    activeFileId: activeFilePane ? parsePaneId(activeFilePane).id : null,
  };
}

export async function restoreWorkspaceLayout(localShell: boolean): Promise<{ layout: WorkspaceLayout | null; warnings: string[] }> {
  const saved = readWorkspaceLayout();
  if (!saved) return { layout: null, warnings: [] };
  const mapping = new Map<string, Session>();
  const used = new Set<string>();
  const warnings: string[] = [];
  for (const descriptor of saved.sessions) {
    const state = useSessionStore.getState();
    let session: Session | null | undefined = state.sessions.find((s) => s.id === descriptor.id && !used.has(s.id));
    // Daemon-backed sessions can survive a GUI restart with new client bookkeeping.
    if (!session && descriptor.sessionType === 'ssh') {
      session = state.sessions.find((s) => !used.has(s.id) && s.sessionType === 'ssh' && s.serverId === descriptor.serverId);
    }
    if (!session) {
      if (descriptor.sessionType === 'ssh') session = await state.connectSession(descriptor.serverName);
      else if (localShell) {
        session = await state.createLocalShellSession(descriptor.serverId, 80, 24);
      }
    }
    if (session) { mapping.set(descriptor.id, session); used.add(session.id); }
    else warnings.push(descriptor.serverName);
  }
  const layout = remapWorkspace(saved, mapping);
  for (const previous of saved.files) {
    const session = mapping.get(previous.sessionId);
    if (!session) continue;
    const buffer = readTextBuffer(previous.id);
    if (buffer) writeTextBuffer(`${session.id}\u0000${previous.path}`, buffer);
  }
  // Keep live sessions not referenced by the saved workspace at the end of the rail.
  useSessionStore.setState((state) => ({ sessions: [...mapping.values(), ...state.sessions.filter((s) => !used.has(s.id))], activeSessionId: layout.activeSessionId ?? state.activeSessionId }));
  useFileWorkspaceStore.setState({ tabs: layout.files, activeTabId: layout.activeFileId });
  saveDetachedLayout(layout.detached);
  return { layout, warnings };
}

export async function restoreWorkspaceWindows(layout: WorkspaceLayout | null): Promise<void> {
  if (layout) await restoreDetachedWindows(layout.detached);
}
