import { FormEvent, useState } from 'react';
import { api } from '../api/client';
import type { TripEmailSender } from '../api/types';

interface Props {
  tripId: string;
  tripName: string;
  initialSenders: TripEmailSender[];
  onChange: () => void | Promise<void>;
}

// Substitute the user's real intake address at build/runtime via a meta tag
// or env var if it varies. For now the canonical address is hard-coded; the
// trip-name prefix is what routes each forward to the right trip.
const INTAKE_ADDRESS = 'trips@in.nathaninge.com';

export default function EmailSenders({
  tripId,
  tripName,
  initialSenders,
  onChange,
}: Props) {
  const [senders, setSenders] = useState<TripEmailSender[]>(initialSenders);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const refresh = async () => {
    setSenders(initialSenders);
    await onChange();
  };

  const add = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setError(null);
    const fd = new FormData(e.currentTarget);
    const email = String(fd.get('email') || '').trim().toLowerCase();
    if (!email || !email.includes('@')) {
      setError('Enter a full email address.');
      return;
    }
    setAdding(true);
    try {
      const created = await api.addEmailSender(tripId, email);
      setSenders((prev) =>
        prev.some((s) => s.email === created.email)
          ? prev
          : [...prev, created].sort((a, b) => a.email.localeCompare(b.email)),
      );
      e.currentTarget.reset();
      await onChange();
    } catch (err) {
      setError(String(err));
    } finally {
      setAdding(false);
    }
  };

  const remove = async (email: string) => {
    if (!confirm(`Stop accepting forwarded confirmations from ${email}?`)) return;
    try {
      await api.removeEmailSender(tripId, email);
      setSenders((prev) => prev.filter((s) => s.email !== email));
      await onChange();
    } catch (err) {
      setError(String(err));
    }
  };

  const copy = (text: string) => navigator.clipboard?.writeText(text);

  return (
    <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
      <div>
        <h2 className="text-lg font-semibold">Forward by email</h2>
        <p className="mt-1 text-xs text-slate-600">
          Anyone on the list below can forward a confirmation to{' '}
          <code className="rounded bg-slate-100 px-1 font-mono text-[11px]">
            {INTAKE_ADDRESS}
          </code>{' '}
          with the trip name on the first line, and it'll land here.
        </p>
      </div>

      <div className="space-y-2 rounded-lg bg-slate-50 p-3 text-sm">
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs uppercase tracking-wide text-slate-500">
            Address
          </span>
          <button
            onClick={() => copy(INTAKE_ADDRESS)}
            className="text-xs text-teal-700 hover:underline"
          >
            Copy
          </button>
        </div>
        <code className="block break-all rounded bg-white px-2 py-1 font-mono text-xs">
          {INTAKE_ADDRESS}
        </code>
        <div className="flex items-center justify-between gap-2 pt-1">
          <span className="text-xs uppercase tracking-wide text-slate-500">
            Body must start with
          </span>
          <button
            onClick={() => copy(tripName)}
            className="text-xs text-teal-700 hover:underline"
          >
            Copy
          </button>
        </div>
        <code className="block break-all rounded bg-white px-2 py-1 font-mono text-xs">
          {tripName}
        </code>
      </div>

      {error && (
        <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      <div>
        <p className="text-xs font-medium uppercase tracking-wide text-slate-500">
          Allowed senders
        </p>
        {senders.length === 0 ? (
          <p className="mt-1 text-sm text-slate-500">
            Nobody yet — add at least one address below.
          </p>
        ) : (
          <ul className="mt-1 space-y-1">
            {senders.map((s) => (
              <li
                key={s.email}
                className="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2 text-sm"
              >
                <span className="font-mono text-xs">{s.email}</span>
                <button
                  onClick={() => remove(s.email)}
                  className="text-sm text-red-700 hover:underline"
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <form onSubmit={add} className="flex gap-2">
        <input
          name="email"
          type="email"
          placeholder="name@example.com"
          className="flex-1"
        />
        <button
          type="submit"
          disabled={adding}
          className="rounded-lg bg-teal-600 px-3 py-2 text-sm font-medium text-white hover:bg-teal-700 disabled:opacity-60"
        >
          {adding ? '…' : 'Add'}
        </button>
      </form>
    </section>
  );
}
