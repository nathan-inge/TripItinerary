import { FormEvent, useCallback, useEffect, useState } from 'react';
import { api } from '../api/client';
import type { ShareScope, TripMember } from '../api/types';

interface Props {
  tripId: string;
}

const ROLES: ShareScope[] = ['view', 'edit'];

const roleLabel = (r: ShareScope) => (r === 'edit' ? 'Can edit' : 'View only');

/**
 * Share a trip with someone who signs in as themselves. Unlike a share link,
 * the trip shows up on their own home page and the grant is tied to their
 * address rather than to a secret in a URL.
 */
export default function TripMembers({ tripId }: Props) {
  const [members, setMembers] = useState<TripMember[]>([]);
  const [email, setEmail] = useState('');
  const [role, setRole] = useState<ShareScope>('view');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setMembers(await api.listMembers(tripId));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [tripId]);

  useEffect(() => {
    load();
  }, [load]);

  const add = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const addr = email.trim().toLowerCase();
    if (!addr.includes('@')) {
      setError('Enter a full email address.');
      return;
    }
    setBusy(true);
    try {
      await api.upsertMember(tripId, { email: addr, role });
      setEmail('');
      setError(null);
      await load();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const changeRole = async (m: TripMember, next: ShareScope) => {
    try {
      await api.upsertMember(tripId, { email: m.email, role: next });
      await load();
    } catch (err) {
      setError(String(err));
    }
  };

  const remove = async (m: TripMember) => {
    if (!confirm(`Remove ${m.email}? They'll lose access immediately.`)) return;
    try {
      await api.removeMember(tripId, m.email);
      await load();
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
      <div>
        <h2 className="text-lg font-semibold">Share with people</h2>
        <p className="mt-1 text-xs text-slate-500">
          They sign in with their own email and this trip appears on their home
          page. Use a share link instead for anyone who shouldn't need an
          account.
        </p>
      </div>

      {error && (
        <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      {members.length === 0 ? (
        <p className="text-sm text-slate-500">Not shared with anyone yet.</p>
      ) : (
        <ul className="space-y-1">
          {members.map((m) => (
            <li
              key={m.email}
              className="flex items-center justify-between gap-2 rounded-lg bg-slate-50 px-3 py-2"
            >
              <div className="min-w-0">
                <p className="truncate font-mono text-xs">{m.email}</p>
                <p className="text-xs text-slate-500">
                  added {m.created_at.slice(0, 10)}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <select
                  value={m.role}
                  onChange={(e) =>
                    changeRole(m, e.target.value as ShareScope)
                  }
                  className="!py-1 text-xs"
                  aria-label={`Access for ${m.email}`}
                >
                  {ROLES.map((r) => (
                    <option key={r} value={r}>
                      {roleLabel(r)}
                    </option>
                  ))}
                </select>
                <button
                  onClick={() => remove(m)}
                  className="text-sm text-red-700 hover:underline"
                >
                  Remove
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <form onSubmit={add} className="grid grid-cols-[1fr_auto_auto] gap-2">
        <input
          type="email"
          placeholder="name@example.com"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
        <select
          value={role}
          onChange={(e) => setRole(e.target.value as ShareScope)}
          aria-label="Access level"
        >
          {ROLES.map((r) => (
            <option key={r} value={r}>
              {roleLabel(r)}
            </option>
          ))}
        </select>
        <button
          type="submit"
          disabled={busy}
          className="rounded-lg bg-teal-600 px-3 py-2 text-sm font-medium text-white hover:bg-teal-700 disabled:opacity-60"
        >
          {busy ? '…' : 'Share'}
        </button>
      </form>

      <p className="rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-800">
        <strong>One more step per person:</strong> the address also has to be on
        the Cloudflare Access policy for this site, or they'll be stopped at the
        login page before the app ever sees them. Zero Trust → Access →
        Applications → <em>tripit owner</em> → policy <em>Owner</em> → add the
        email to the Include list.
      </p>
    </section>
  );
}
