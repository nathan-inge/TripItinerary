import { useCallback, useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { api } from '../api/client';
import type { CreateItem, Item, TripDetail } from '../api/types';
import Timeline from '../components/Timeline';
import AddItemForm from '../components/AddItemForm';

export default function TripPage() {
  const { id } = useParams<{ id: string }>();
  const [data, setData] = useState<TripDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const load = useCallback(async () => {
    if (!id) return;
    try {
      setData(await api.getTrip(id));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  const handleAdd = async (body: CreateItem) => {
    if (!id) return;
    await api.createItem(id, body);
    setAdding(false);
    load();
  };

  const handleDeleteItem = async (item: Item) => {
    if (!confirm(`Delete "${item.title}"?`)) return;
    await api.deleteItem(item.id);
    load();
  };

  const handleSaveDayNote = async (date: string, notes_md: string) => {
    if (!id) return;
    await api.upsertDayNote(id, date, notes_md);
    load();
  };

  if (!data) {
    return error ? (
      <p className="text-red-700">{error}</p>
    ) : (
      <p className="text-slate-500">Loading…</p>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <Link to="/" className="text-sm text-teal-700 hover:underline">
            ← Trips
          </Link>
          <h1 className="text-2xl font-bold">{data.name}</h1>
          <p className="text-sm text-slate-600">
            {data.start_date} → {data.end_date}
            {data.destination && <> · {data.destination}</>}
          </p>
        </div>
        <Link
          to={`/trips/${data.id}/manage`}
          className="rounded-lg border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-100"
        >
          Manage
        </Link>
      </div>

      <div>
        <button
          onClick={() => setAdding((v) => !v)}
          className="rounded-lg bg-teal-600 px-3 py-2 text-sm font-medium text-white hover:bg-teal-700"
        >
          {adding ? 'Cancel' : '+ Add item'}
        </button>
      </div>

      {adding && (
        <AddItemForm
          travelers={data.travelers}
          onCancel={() => setAdding(false)}
          onSubmit={handleAdd}
        />
      )}

      <Timeline
        trip={data}
        items={data.items}
        dayNotes={data.day_notes}
        onSaveDayNote={handleSaveDayNote}
        onDeleteItem={handleDeleteItem}
      />
    </div>
  );
}

