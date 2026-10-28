import { useCallback, useEffect, useState } from 'react';
import { api } from '../api/client';
import type { CreatedShareLink, ShareLink, ShareScope } from '../api/types';

interface Props {
  tripId: string;
}

const SCOPES: ShareScope[] = ['view', 'edit'];

export default function ShareLinkManager({ tripId }: Props) {
  const [links, setLinks] = useState<ShareLink[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [label, setLabel] = useState('');
  const [scope, setScope] = useState<ShareScope>('view');
  const [justCreated, setJustCreated] = useState<CreatedShareLink | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    try {
      setLinks(await api.listShareLinks(tripId));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [tripId]);

  useEffect(() => {
    load();
  }, [load]);

  const mint = async () => {
    setCreating(true);
    try {
      const c = await api.createShareLink(tripId, {
        label: label.trim() || null,
        scope,
      });
      setJustCreated(c);
      setLabel('');
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  };

  const revoke = async (l: ShareLink) => {
    if (!confirm(`Revoke ${l.label || l.scope + ' link'}? It will stop working immediately.`))
      return;
    try {
      await api.revokeShareLink(l.id);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  const fullUrl = (token: string) =>
    `${window.location.origin}/share/${token}`;

  return (
    <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
      <div>
        <h2 className="text-lg font-semibold">Share links</h2>
        <p className="text-xs text-slate-500">
          Anyone with the link gets access at the chosen scope, no account
          needed. Revoke anytime.
        </p>
      </div>

      {error && (
        <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      {justCreated && (
        <div className="space-y-2 rounded-lg border border-teal-300 bg-teal-50 p-3">
          <p className="text-sm font-medium text-teal-900">
            New {justCreated.scope} link created. Copy it now — it won't be shown
            again.
          </p>
          <div className="flex items-center gap-2">
            <input
              readOnly
              value={fullUrl(justCreated.token)}
              onFocus={(e) => e.currentTarget.select()}
              className="font-mono text-xs"
            />
            <button
              onClick={() =>
                navigator.clipboard.writeText(fullUrl(justCreated.token))
              }
              className="rounded-lg bg-teal-700 px-3 py-2 text-sm font-medium text-white hover:bg-teal-800"
            >
              Copy
            </button>
          </div>
          <button
            onClick={() => setJustCreated(null)}
            className="text-xs text-teal-700 hover:underline"
          >
            Dismiss
          </button>
        </div>
      )}

      <div className="grid grid-cols-[1fr_auto_auto] gap-2">
        <input
          placeholder="Label (e.g. Em's edit link)"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
        />
        <select
          value={scope}
          onChange={(e) => setScope(e.target.value as ShareScope)}
        >
          {SCOPES.map((s) => (
            <option key={s} value={s}>
              {s === 'view' ? 'View only' : 'Edit'}
            </option>
          ))}
        </select>
        <button
          onClick={mint}
          disabled={creating}
          className="rounded-lg bg-teal-600 px-3 py-2 text-sm font-medium text-white hover:bg-teal-700 disabled:opacity-60"
        >
          {creating ? '…' : 'Mint'}
        </button>
      </div>

      {links.length === 0 ? (
        <p className="text-sm text-slate-500">No share links yet.</p>
      ) : (
        <ul className="space-y-1">
          {links.map((l) => (
            <li
              key={l.id}
              className={`flex items-center justify-between rounded-lg px-3 py-2 ${
                l.revoked_at ? 'bg-slate-100 text-slate-500' : 'bg-slate-50'
              }`}
            >
              <div className="min-w-0">
                <div className="flex items-baseline gap-2">
                  <span className="font-medium">{l.label || '(unlabeled)'}</span>
                  <span className="text-xs uppercase tracking-wide text-slate-500">
                    {l.scope}
                  </span>
                  {l.revoked_at && (
                    <span className="text-xs uppercase text-red-600">
                      revoked
                    </span>
                  )}
                </div>
                <p className="text-xs text-slate-500">
                  created {l.created_at.slice(0, 10)}
                  {l.last_used_at && (
                    <> · last used {l.last_used_at.slice(0, 10)}</>
                  )}
                </p>
              </div>
              {!l.revoked_at && (
                <button
                  onClick={() => revoke(l)}
                  className="text-sm text-red-700 hover:underline"
                >
                  Revoke
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
