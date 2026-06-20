import { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { shareApi } from '../api/client';
import type { CreateItem, Item, SharedTripDetail } from '../api/types';
import Timeline from '../components/Timeline';
import AddItemForm from '../components/AddItemForm';
import PasteParseForm from '../components/PasteParseForm';

type AddMode = 'closed' | 'manual' | 'paste';

export default function SharedTrip() {
  const { token } = useParams<{ token: string }>();
  const [data, setData] = useState<SharedTripDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [addMode, setAddMode] = useState<AddMode>('closed');

  const client = token ? shareApi(token) : null;

  const load = useCallback(async () => {
    if (!client) return;
    try {
      setData(await client.getTrip());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  useEffect(() => {
    load();
  }, [load]);

  const canEdit = data?.share_scope === 'edit';

  const handleAdd = async (body: CreateItem) => {
    if (!client) return;
    await client.createItem(body);
    setAddMode('closed');
    load();
  };

  const handleDeleteItem = async (item: Item) => {
    if (!client) return;
    if (!confirm(`Delete "${item.title}"?`)) return;
    await client.deleteItem(item.id);
    load();
  };

  const handleUpdateItem = async (item: Item, body: CreateItem) => {
    if (!client) return;
    await client.updateItem(item.id, body);
    load();
  };

  const handleSaveDayNote = async (date: string, notes_md: string) => {
    if (!client) return;
    await client.upsertDayNote(date, notes_md);
    load();
  };

  if (!data) {
    return error ? (
      <div className="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-800">
        <p className="font-medium">This link doesn't work.</p>
        <p className="mt-1 text-xs">
          It may have been revoked or mistyped. Ask the trip owner for a fresh
          link.
        </p>
      </div>
    ) : (
      <p className="text-slate-500">Loading…</p>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-bold">{data.name}</h1>
        <p className="text-sm text-slate-600">
          {data.start_date} → {data.end_date}
          {data.destination && <> · {data.destination}</>}
          <span className="ml-2 rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium uppercase text-slate-700">
            {data.share_scope === 'edit' ? 'editor' : 'view only'}
          </span>
        </p>
      </div>

      {canEdit && token && (
        <>
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() =>
                setAddMode((m) => (m === 'paste' ? 'closed' : 'paste'))
              }
              className={`rounded-lg px-3 py-2 text-sm font-medium ${
                addMode === 'paste'
                  ? 'bg-slate-200 text-slate-700'
                  : 'bg-teal-600 text-white hover:bg-teal-700'
              }`}
            >
              {addMode === 'paste' ? 'Cancel' : '📋 Paste confirmation'}
            </button>
            <button
              onClick={() =>
                setAddMode((m) => (m === 'manual' ? 'closed' : 'manual'))
              }
              className={`rounded-lg px-3 py-2 text-sm font-medium ${
                addMode === 'manual'
                  ? 'bg-slate-200 text-slate-700'
                  : 'border border-slate-300 bg-white text-slate-700 hover:bg-slate-100'
              }`}
            >
              {addMode === 'manual' ? 'Cancel' : '+ Add manually'}
            </button>
          </div>

          {addMode === 'paste' && (
            <SharedPasteForm
              token={token}
              travelers={data.travelers}
              onCancel={() => setAddMode('closed')}
              onSaved={async () => {
                setAddMode('closed');
                await load();
              }}
            />
          )}
          {addMode === 'manual' && (
            <AddItemForm
              travelers={data.travelers}
              onCancel={() => setAddMode('closed')}
              onSubmit={handleAdd}
            />
          )}
        </>
      )}

      <Timeline
        trip={data}
        items={data.items}
        dayNotes={data.day_notes}
        onSaveDayNote={handleSaveDayNote}
        onDeleteItem={handleDeleteItem}
        onUpdateItem={canEdit ? handleUpdateItem : undefined}
        readOnly={!canEdit}
      />
    </div>
  );
}

// Thin wrapper that swaps the API client for the share-token variant.
function SharedPasteForm(props: {
  token: string;
  travelers: SharedTripDetail['travelers'];
  onCancel: () => void;
  onSaved: () => Promise<void>;
}) {
  // PasteParseForm uses the owner api; rather than parameterize it on a client,
  // we duplicate the small surface here by calling shareApi directly through
  // a wrapper component. Cleaner alternative would be a context — overkill for v1.
  return (
    <PasteParseFormShared
      api={shareApi(props.token)}
      travelers={props.travelers}
      onCancel={props.onCancel}
      onSaved={props.onSaved}
    />
  );
}

// Copy of PasteParseForm parameterized on an api object. Kept inline to avoid
// premature abstraction across owner/share variants.
function PasteParseFormShared({
  api,
  travelers,
  onCancel,
  onSaved,
}: {
  api: ReturnType<typeof shareApi>;
  travelers: SharedTripDetail['travelers'];
  onCancel: () => void;
  onSaved: () => void | Promise<void>;
}) {
  const [rawText, setRawText] = useState('');
  const [parsing, setParsing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<{
    initial: Partial<CreateItem>;
    reviewReason: string;
  } | null>(null);

  const parse = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setError(null);
    if (!rawText.trim()) return;
    setParsing(true);
    try {
      const resp = await api.parseText(rawText);
      setPreview({
        initial: {
          type: resp.preview.type,
          title: resp.preview.title,
          start_at: resp.preview.start_at,
          end_at: resp.preview.end_at,
          location: resp.preview.location,
          confirmation_no: resp.preview.confirmation_no,
          notes: resp.preview.notes,
          details: resp.preview.details,
          links: resp.preview.links,
        },
        reviewReason: resp.review_reason,
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setParsing(false);
    }
  };

  const save = async (body: CreateItem) => {
    await api.createItem(body);
    await onSaved();
  };

  if (preview) {
    return (
      <div className="space-y-3">
        <p className="text-sm text-slate-600">
          Parsed preview — review & edit before saving.
        </p>
        <AddItemForm
          travelers={travelers}
          initial={preview.initial}
          reviewWarning={preview.reviewReason || undefined}
          onCancel={() => setPreview(null)}
          onSubmit={save}
        />
      </div>
    );
  }

  return (
    <form
      onSubmit={parse}
      className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm"
    >
      <label className="block">
        <span className="text-sm font-medium text-slate-700">
          Paste a confirmation email
        </span>
        <textarea
          rows={10}
          required
          value={rawText}
          onChange={(e) => setRawText(e.target.value)}
          placeholder="Paste the full email body. The parser will extract a structured item for you to review and edit."
          className="font-mono text-xs"
        />
      </label>
      {error && (
        <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={parsing}
          className="rounded-lg bg-teal-600 px-4 py-2 text-sm font-medium text-white hover:bg-teal-700 disabled:opacity-60"
        >
          {parsing ? 'Parsing…' : 'Parse'}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-lg border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-100"
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
