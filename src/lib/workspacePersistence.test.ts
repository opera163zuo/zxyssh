// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readWorkspaceLayout, remapWorkspace, saveWorkspaceLayout, type WorkspaceLayout } from './workspacePersistence';
import { useSessionStore, type Session } from '../stores/sessionStore';
import { useFileWorkspaceStore } from '../stores/fileWorkspaceStore';
import { filePaneId, sessionPaneId } from './paneIds';
import { getLeaves } from './mosaicTree';

const session: Session = { id: 'new', serverId: 'server', serverName: 'Host', sessionType: 'ssh', state: 'disconnected', createdAt: 1788739200000, purpose: 'shell' };
function fixture(): WorkspaceLayout {
  const file = { id: 'old\u0000/srv/file.ts', sessionId: 'old', path: '/srv/file.ts', name: 'file.ts', size: 12, kind: 'text' as const, dirty: true };
  return { version: 2,
    tree: { direction: 'row', first: sessionPaneId('old'), second: filePaneId(file.id), splitPercentage: 42.75 },
    focusedPane: filePaneId(file.id), sessions: [{ ...session, id: 'old' }], files: [file],
    activeSessionId: 'old', activeFileId: file.id,
    detached: [{ target: { kind: 'terminal', sessionId: 'old', title: 'Host' }, geometryKey: 'stable-screen-position' }],
  };
}

describe('persistent workspace identity', () => {
  beforeEach(() => {
    vi.unstubAllGlobals();
    const values = new Map<string, string>();
    vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value), removeItem: (key: string) => values.delete(key) });
    useSessionStore.setState({ sessions: [], activeSessionId: null });
    useFileWorkspaceStore.setState({ tabs: [], activeTabId: null });
  });
  it('remaps session, file, focus and detached references while preserving ratios', () => {
    const next = remapWorkspace(fixture(), new Map([['old', session]]));
    expect(getLeaves(next.tree)).toEqual([sessionPaneId('new'), filePaneId('new\u0000/srv/file.ts')]);
    expect(next.tree).toMatchObject({ splitPercentage: 42.75 });
    expect(next.focusedPane).toBe(filePaneId('new\u0000/srv/file.ts'));
    expect(next.activeFileId).toBe('new\u0000/srv/file.ts');
    expect(next.activeSessionId).toBe('new');
    expect(next.detached[0]).toMatchObject({ target: { sessionId: 'new' }, geometryKey: 'stable-screen-position' });
  });
  it('collapses unavailable sessions instead of restoring dead references', () => {
    const next = remapWorkspace(fixture(), new Map());
    expect(next.tree).toBeNull(); expect(next.files).toEqual([]); expect(next.detached).toEqual([]);
    expect(next.focusedPane).toBeNull(); expect(next.activeFileId).toBeNull();
  });
  it('saves tab order and split layout without credentials', () => {
    const original = fixture();
    useSessionStore.setState({ sessions: [{ ...session, password: 'never-store-password' } as Session], activeSessionId: 'new' });
    useFileWorkspaceStore.setState({ tabs: original.files, activeTabId: original.activeFileId });
    saveWorkspaceLayout(original.tree, original.focusedPane);
    const raw = localStorage.getItem('zxyssh.workspace-layout.v2')!;
    expect(raw).not.toContain('never-store-password');
    expect(readWorkspaceLayout()).toMatchObject({ tree: original.tree, files: original.files, activeFileId: original.activeFileId });
  });
  it.each(['invalid-json', '{}', 'null', '{"version":1}', '{"version":2,"sessions":[]}'])('ignores invalid persisted data %s', (raw) => {
    localStorage.setItem('zxyssh.workspace-layout.v2', raw);
    expect(readWorkspaceLayout()).toBeNull();
  });
});
