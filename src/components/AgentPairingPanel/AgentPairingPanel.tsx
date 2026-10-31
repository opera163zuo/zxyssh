import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { KeyRound, Link2, RefreshCw, ShieldCheck, Trash2 } from 'lucide-react';

import {
  useMcpPairingStore,
  type PairedAgent,
} from '../../stores/mcpPairingStore';

const POLL_INTERVAL_MS = 2_000;

function remainingSeconds(expiresAt: number, now: number): number {
  return Math.max(0, Math.ceil(expiresAt - now / 1000));
}

export function AgentPairingPanel() {
  const { t } = useTranslation();
  const code = useMcpPairingStore((state) => state.code);
  const agents = useMcpPairingStore((state) => state.agents);
  const lanStatus = useMcpPairingStore((state) => state.lanStatus);
  const issuedToken = useMcpPairingStore((state) => state.issuedToken);
  const busy = useMcpPairingStore((state) => state.busy);
  const error = useMcpPairingStore((state) => state.error);
  const refresh = useMcpPairingStore((state) => state.refresh);
  const createCode = useMcpPairingStore((state) => state.createCode);
  const checkCode = useMcpPairingStore((state) => state.checkCode);
  const pairedJustNow = useMcpPairingStore((state) => state.pairedJustNow);
  const revoke = useMcpPairingStore((state) => state.revoke);
  const rename = useMcpPairingStore((state) => state.rename);
  const rotate = useMcpPairingStore((state) => state.rotate);
  const dismissIssuedToken = useMcpPairingStore((state) => state.dismissIssuedToken);

  const [now, setNow] = useState(() => Date.now());
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // A code can be spent from another process, so the countdown is re-read live.
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, []);

  // Spending the code is also how a pairing succeeds, so the same question
  // doubles as the "someone just paired" signal. `now` is deliberately not a
  // dependency: putting it here rebuilt the timer every second and it never
  // fired.
  useEffect(() => {
    if (code === null) return;
    const timer = window.setInterval(() => {
      void checkCode();
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [code, checkCode]);

  const codeRemaining = code ? remainingSeconds(code.expiresAt, now) : 0;

  return (
    <section className="border-y border-tokyo-bg-hl bg-tokyo-bg-dark/50">
      <header className="flex items-center justify-between gap-4 border-b border-tokyo-bg-hl px-4 py-3">
        <div className="min-w-0">
          <h3 className="text-sm font-medium text-tokyo-fg">{t('settings.pairingTitle')}</h3>
          <p className="text-xs text-tokyo-comment">{t('settings.pairingDesc')}</p>
        </div>
        <button
          type="button"
          className="icon-button"
          onClick={() => void refresh()}
          disabled={busy}
          aria-label={t('settings.pairingRefresh')}
          title={t('settings.pairingRefresh')}
        >
          <RefreshCw className={`h-4 w-4 ${busy ? 'animate-spin' : ''}`} />
        </button>
      </header>

      <dl className="grid gap-1 border-b border-tokyo-bg-hl px-4 py-3 text-sm sm:grid-cols-[10rem_minmax(0,1fr)]">
        <dt className="text-tokyo-comment">{t('settings.pairingListener')}</dt>
        <dd className="flex flex-wrap items-center gap-2 text-tokyo-fg">
          <span
            className={`h-2 w-2 rounded-full ${lanStatus ? 'bg-tokyo-green' : 'bg-tokyo-red'}`}
          />
          {lanStatus ? (
            <span className="font-mono text-xs">{lanStatus.bind}</span>
          ) : (
            <span className="text-tokyo-comment">{t('settings.pairingListenerStopped')}</span>
          )}
          {lanStatus?.credentialSealing && (
            <span className="flex items-center gap-1 rounded border border-tokyo-bg-hl px-1.5 py-0.5 text-xs text-tokyo-blue">
              <ShieldCheck className="h-3 w-3" />
              {t('settings.pairingSealed')}
            </span>
          )}
          {lanStatus && (
            <span className="text-xs text-tokyo-comment">
              {t('settings.pairingAgentsCount', {
                agents: lanStatus.activeAgents,
              })}
            </span>
          )}
        </dd>

        <dt className="text-tokyo-comment">{t('settings.pairingCode')}</dt>
        <dd className="text-tokyo-fg">
          {code && codeRemaining > 0 ? (
            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="rounded border border-tokyo-bg-hl bg-tokyo-bg px-3 py-1 font-mono text-lg tracking-widest text-tokyo-yellow hover:border-tokyo-yellow"
                onClick={() => void createCode()}
                disabled={busy}
                title={t('settings.pairingCodeReroll')}
              >
                {code.code}
              </button>
              <span className="text-xs text-tokyo-comment">
                {t('settings.pairingCodeExpires', { seconds: codeRemaining })}
              </span>
            </div>
          ) : (
            <div className="flex flex-col gap-1">
              <button
                type="button"
                className="icon-button w-fit gap-2"
                onClick={() => void createCode()}
                disabled={busy}
              >
                <KeyRound className="h-4 w-4" />
                {t('settings.pairingCodeCreate')}
              </button>
              <span className="text-xs text-tokyo-comment">{t('settings.pairingCodeHint')}</span>
            </div>
          )}
        </dd>
      </dl>

      {pairedJustNow && (
        <p className="flex items-center gap-2 border-b border-tokyo-bg-hl bg-tokyo-green/10 px-4 py-2 text-xs text-tokyo-green">
          <ShieldCheck className="h-3.5 w-3.5" />
          {t('settings.pairingJustPaired', { name: pairedJustNow })}
        </p>
      )}

      {error && (
        <p className="border-b border-tokyo-bg-hl px-4 py-2 text-xs text-tokyo-red">{error}</p>
      )}

      {issuedToken && (
        <div className="flex flex-wrap items-center gap-3 border-b border-tokyo-bg-hl bg-tokyo-bg/60 px-4 py-3">
          <span className="text-xs text-tokyo-yellow">{t('settings.pairingTokenOnce')}</span>
          <code className="break-all font-mono text-xs text-tokyo-fg">{issuedToken}</code>
          <button type="button" className="icon-button" onClick={dismissIssuedToken}>
            {t('settings.pairingTokenDone')}
          </button>
        </div>
      )}

      <div className="px-4 py-3">
        <h4 className="text-xs font-medium uppercase tracking-wide text-tokyo-comment">
          {t('settings.pairingAgents')}
        </h4>
        {agents.length === 0 ? (
          <p className="mt-2 text-xs text-tokyo-comment">{t('settings.pairingNoAgents')}</p>
        ) : (
          <ul className="mt-2 space-y-2">
            {agents.map((agent) => (
              <AgentRow
                key={agent.id}
                agent={agent}
                busy={busy}
                editing={editing?.id === agent.id ? editing.name : null}
                onEdit={(name) => setEditing({ id: agent.id, name })}
                onCancelEdit={() => setEditing(null)}
                onSaveName={(name) => {
                  setEditing(null);
                  void rename(agent.id, name);
                }}
                onRevoke={() => void revoke(agent.id)}
                onRotate={() => void rotate(agent.id)}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}


function AgentRow({
  agent,
  busy,
  editing,
  onEdit,
  onCancelEdit,
  onSaveName,
  onRevoke,
  onRotate,
}: {
  agent: PairedAgent;
  busy: boolean;
  editing: string | null;
  onEdit: (name: string) => void;
  onCancelEdit: () => void;
  onSaveName: (name: string) => void;
  onRevoke: () => void;
  onRotate: () => void;
}) {
  const { t } = useTranslation();
  return (
    <li className="flex flex-wrap items-center justify-between gap-3 rounded border border-tokyo-bg-hl bg-tokyo-bg px-3 py-2">
      <div className="min-w-0">
        {editing !== null ? (
          <form
            className="flex items-center gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              const value = new FormData(event.currentTarget).get('name');
              const name = typeof value === 'string' ? value.trim() : '';
              if (name) onSaveName(name);
              else onCancelEdit();
            }}
          >
            <input
              name="name"
              defaultValue={editing}
              className="w-40 rounded border border-tokyo-bg-hl bg-tokyo-bg-dark px-2 py-1 text-sm text-tokyo-fg"
              autoFocus
            />
            <button type="submit" className="icon-button">
              {t('settings.pairingSave')}
            </button>
            <button type="button" className="icon-button" onClick={onCancelEdit}>
              {t('common.cancel')}
            </button>
          </form>
        ) : (
          <div className="flex items-center gap-2">
            <span className="truncate text-sm text-tokyo-fg">{agent.name}</span>
            {agent.revoked && (
              <span className="rounded border border-tokyo-red px-1.5 text-xs text-tokyo-red">
                {t('settings.pairingRevoked')}
              </span>
            )}
          </div>
        )}
        <div className="flex items-center gap-1 font-mono text-xs text-tokyo-comment">
          <Link2 className="h-3 w-3" />
          {agent.id.slice(0, 8)}
        </div>
      </div>
      {!agent.revoked && (
        <div className="flex items-center gap-2">
          <button type="button" className="icon-button" disabled={busy} onClick={() => onEdit(agent.name)}>
            {t('settings.pairingRename')}
          </button>
          <button type="button" className="icon-button" disabled={busy} onClick={onRotate}>
            {t('settings.pairingRotate')}
          </button>
          <button type="button" className="icon-button" disabled={busy} onClick={onRevoke}>
            <Trash2 className="h-4 w-4 text-tokyo-red" />
            {t('settings.pairingRevoke')}
          </button>
        </div>
      )}
    </li>
  );
}
