import { Folder, FolderOpen, Pencil, Plus, Server as ServerIcon } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '../../lib/utils';
import { useServerStore, type Group, type Server } from '../../stores/serverStore';

interface ServerSidebarProps {
  connectedServerIds: Set<string>;
  onConnect: (server: Server) => void;
  onAddServer: () => void;
  onEditServer: (server: Server) => void;
}

export function ServerSidebar({
  connectedServerIds,
  onConnect,
  onAddServer,
  onEditServer,
}: ServerSidebarProps) {
  const { t } = useTranslation();
  const servers = useServerStore((state) => state.servers) ?? [];
  const groups = useServerStore((state) => state.groups) ?? [];
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const childrenByParent = useMemo(() => {
    const result = new Map<string | null, Group[]>();
    for (const group of groups) {
      const key = group.parent_id ?? null;
      const children = result.get(key) ?? [];
      children.push(group);
      result.set(key, children);
    }
    return result;
  }, [groups]);

  const toggleGroup = (groupId: string) => {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(groupId)) next.delete(groupId);
      else next.add(groupId);
      return next;
    });
  };

  const renderServers = (groupId: string | null, depth: number): JSX.Element[] => {
    const groupServers = servers.filter((server) => (server.group_id ?? null) === groupId);
    const groupChildren = childrenByParent.get(groupId) ?? [];
    const rows: JSX.Element[] = [];

    for (const group of groupChildren) {
      const isCollapsed = collapsed.has(group.id);
      rows.push(
        <div key={`group-${group.id}`}>
          <button
            type="button"
            className="flex w-full items-center gap-2 px-2 py-1.5 text-left text-xs text-tokyo-fg hover:bg-tokyo-bg-hl"
            style={{ paddingLeft: `${8 + depth * 14}px` }}
            onClick={() => toggleGroup(group.id)}
          >
            {isCollapsed ? <Folder className="h-3.5 w-3.5" /> : <FolderOpen className="h-3.5 w-3.5" />}
            <span className="truncate">{group.name}</span>
          </button>
          {!isCollapsed && renderServers(group.id, depth + 1)}
        </div>
      );
    }

    for (const server of groupServers) {
      const connected = connectedServerIds.has(server.id);
      rows.push(
        <div key={server.id} className="group flex items-center gap-1 px-2 py-1" style={{ paddingLeft: `${8 + depth * 14}px` }}>
          <button
            type="button"
            className="flex min-w-0 flex-1 items-center gap-2 rounded px-1.5 py-1 text-left text-sm text-tokyo-fg hover:bg-tokyo-bg-hl"
            onClick={() => onConnect(server)}
            title={`${server.name} (${server.host}:${server.port})`}
          >
            <span className={cn('h-2 w-2 flex-shrink-0 rounded-full', connected ? 'bg-tokyo-green' : 'bg-tokyo-comment')} />
            <ServerIcon className="h-3.5 w-3.5 flex-shrink-0 text-tokyo-blue" />
            <span className="truncate">{server.name}</span>
          </button>
          <button
            type="button"
            className="icon-button tooltip-button opacity-0 group-hover:opacity-100"
            data-tooltip={t('common.edit')}
            aria-label={`${t('common.edit')} ${server.name}`}
            onClick={() => onEditServer(server)}
          >
            <Pencil className="h-3 w-3" />
          </button>
        </div>
      );
    }

    return rows;
  };

  return (
    <aside className="hidden w-64 flex-shrink-0 flex-col border-r border-tokyo-bg-hl bg-tokyo-bg-dark lg:flex" aria-label="SSH sites">
      <div className="flex h-11 items-center justify-between border-b border-tokyo-bg-hl px-3">
        <span className="text-sm font-semibold text-tokyo-fg">SSH Sites</span>
        <button
          type="button"
          className="icon-button tooltip-button"
          data-tooltip={t('server.addServer')}
          aria-label={t('server.add')}
          onClick={onAddServer}
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto py-2">
        {servers.length === 0 ? (
          <button type="button" className="mx-3 mt-2 text-left text-xs text-tokyo-comment hover:text-tokyo-fg" onClick={onAddServer}>
            Add an SSH site to get started.
          </button>
        ) : (
          renderServers(null, 0)
        )}
      </div>
      <button
        type="button"
        className="m-2 flex items-center justify-center gap-2 rounded border border-tokyo-bg-hl px-3 py-2 text-xs text-tokyo-fg hover:bg-tokyo-bg-hl"
        onClick={onAddServer}
      >
        <Plus className="h-3.5 w-3.5" />
        {t('server.addServer')}
      </button>
    </aside>
  );
}
