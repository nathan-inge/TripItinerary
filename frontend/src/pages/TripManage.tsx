import { FormEvent, useCallback, useEffect, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { api } from '../api/client';
import type { TripDetail } from '../api/types';
import ShareLinkManager from '../components/ShareLinkManager';
import TripMembers from '../components/TripMembers';
import EmailSenders from '../components/EmailSenders';

export default function TripManage() {
  const { id } = useParams<{ id: string }>();
  const nav = useNavigate();
  const [data, setData] = useState<TripDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!id) return;
    try {
      setData(await api.getTrip(id));
    } catch (e) {
      setError(String(e));
    }
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  const addTraveler = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!id) return;
    const form = e.currentTarget;
    const fd = new FormData(form);
    const name = String(fd.get('name') || '').trim();
    if (!name) return;
    await api.createTraveler(id, name);
    form.reset();
    load();
  };

  const removeTraveler = async (tid: string) => {
    await api.deleteTraveler(tid);
    load();
  };

  const deleteTrip = async () => {
    if (!id) return;
    if (!confirm('Delete this trip and all its items? This cannot be undone.'))
      return;
    await api.deleteTrip(id);
    nav('/');
  };

  if (!data) {
    return error ? (
      <p className="text-red-700">{error}</p>
    ) : (
      <p className="text-slate-500">Loading…</p>
    );
  }

  // Sharing, the sender allow-list, and deletion stay with the owner. Edit
  // members get travelers, since that's trip content like items and notes.
  const isOwner = data.my_role === 'owner';
  const canEdit = data.my_role !== 'view';

  return (
    <div className="space-y-6">
      <div>
        <Link to={`/trips/${data.id}`} className="text-sm text-teal-700 hover:underline">
          ← {data.name}
        </Link>
        <h1 className="text-2xl font-bold">Manage</h1>
        {!isOwner && (
          <p className="mt-1 text-xs text-slate-500">
            Shared by {data.owner_email} — sharing and deletion are the owner's.
          </p>
        )}
      </div>

      {!canEdit && (
        <p className="rounded-lg bg-slate-100 px-3 py-2 text-sm text-slate-600">
          You have view-only access to this trip, so there's nothing to manage
          here.
        </p>
      )}

      {canEdit && (
        <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
          <h2 className="text-lg font-semibold">Travelers</h2>
          {data.travelers.length === 0 ? (
            <p className="text-sm text-slate-500">No travelers yet.</p>
          ) : (
            <ul className="space-y-1">
              {data.travelers.map((t) => (
                <li
                  key={t.id}
                  className="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2"
                >
                  <span>{t.name}</span>
                  <button
                    onClick={() => removeTraveler(t.id)}
                    className="text-sm text-red-700 hover:underline"
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
          <form onSubmit={addTraveler} className="flex gap-2">
            <input name="name" placeholder="Name" className="flex-1" />
            <button
              type="submit"
              className="rounded-lg bg-teal-600 px-3 py-2 text-sm font-medium text-white hover:bg-teal-700"
            >
              Add
            </button>
          </form>
        </section>
      )}

      {isOwner && (
      <>
        <TripMembers tripId={data.id} />

        <ShareLinkManager tripId={data.id} />

        <EmailSenders
          tripId={data.id}
          tripName={data.name}
          initialSenders={data.email_senders}
          onChange={load}
        />

        <section className="space-y-3 rounded-xl border border-red-200 bg-red-50/50 p-4">
          <h2 className="text-lg font-semibold text-red-800">Danger zone</h2>
          <p className="text-sm text-red-700">
            Deleting the trip removes all its items, travelers, day notes, and share
            links. This cannot be undone.
          </p>
          <button
            onClick={deleteTrip}
            className="rounded-lg bg-red-600 px-3 py-2 text-sm font-medium text-white hover:bg-red-700"
          >
            Delete trip
          </button>
        </section>
      </>
      )}
    </div>
  );
}
