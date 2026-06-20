import { FormEvent, useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';
import type { Trip } from '../api/types';

export default function TripList() {
  const [trips, setTrips] = useState<Trip[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = async () => {
    setLoading(true);
    try {
      setTrips(await api.listTrips());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    reload();
  }, []);

  const submit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const form = e.currentTarget;
    const fd = new FormData(form);
    const name = String(fd.get('name') || '').trim();
    const destination = String(fd.get('destination') || '').trim() || null;
    const start_date = String(fd.get('start_date') || '');
    const end_date = String(fd.get('end_date') || '');
    if (!name || !start_date || !end_date) return;
    try {
      await api.createTrip({ name, destination, start_date, end_date });
      form.reset();
      setCreating(false);
      reload();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Trips</h1>
        <button
          onClick={() => setCreating((v) => !v)}
          className="rounded-lg bg-teal-600 px-3 py-2 text-sm font-medium text-white hover:bg-teal-700"
        >
          {creating ? 'Cancel' : 'New trip'}
        </button>
      </div>

      {error && (
        <div className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </div>
      )}

      {creating && (
        <form
          onSubmit={submit}
          className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm"
        >
          <label className="block">
            <span className="text-sm font-medium text-slate-700">Name</span>
            <input name="name" required placeholder="Japan, autumn 2026" />
          </label>
          <label className="block">
            <span className="text-sm font-medium text-slate-700">
              Destination <span className="text-slate-400">(optional)</span>
            </span>
            <input name="destination" placeholder="Tokyo → Kyoto → Osaka" />
          </label>
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="text-sm font-medium text-slate-700">Start</span>
              <input name="start_date" type="date" required />
            </label>
            <label className="block">
              <span className="text-sm font-medium text-slate-700">End</span>
              <input name="end_date" type="date" required />
            </label>
          </div>
          <button
            type="submit"
            className="rounded-lg bg-teal-600 px-4 py-2 text-sm font-medium text-white hover:bg-teal-700"
          >
            Create
          </button>
        </form>
      )}

      {loading ? (
        <p className="text-slate-500">Loading…</p>
      ) : trips.length === 0 ? (
        <p className="text-slate-500">No trips yet. Create your first one above.</p>
      ) : (
        <ul className="space-y-2">
          {trips.map((t) => (
            <li
              key={t.id}
              className="rounded-xl border border-slate-200 bg-white shadow-sm hover:border-teal-300"
            >
              <Link to={`/trips/${t.id}`} className="block px-4 py-3">
                <div className="flex items-baseline justify-between gap-3">
                  <span className="text-base font-semibold">{t.name}</span>
                  <span className="text-sm tabular-nums text-slate-500">
                    {t.start_date} → {t.end_date}
                  </span>
                </div>
                {t.destination && (
                  <p className="mt-1 text-sm text-slate-600">{t.destination}</p>
                )}
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
