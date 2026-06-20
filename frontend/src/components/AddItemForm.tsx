import { FormEvent, useState } from 'react';
import type { CreateItem, ItemType, Link, Traveler } from '../api/types';

interface Props {
  travelers: Traveler[];
  onCancel: () => void;
  onSubmit: (body: CreateItem) => void | Promise<void>;
  initial?: Partial<CreateItem>;
  reviewWarning?: string;
}

const TYPES: ItemType[] = ['flight', 'lodging', 'transport', 'activity'];

const TYPE_LABEL: Record<ItemType, string> = {
  flight: 'Flight ✈',
  lodging: 'Lodging 🛏',
  transport: 'Transport 🚆',
  activity: 'Activity 🎫',
};

// datetime-local accepts "YYYY-MM-DDTHH:MM"; trim seconds/offset from ISO inputs.
function toLocalInput(iso: string | null | undefined): string {
  if (!iso) return '';
  const m = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})/.exec(iso);
  return m ? `${m[1]}T${m[2]}` : '';
}

export default function AddItemForm({
  travelers,
  onCancel,
  onSubmit,
  initial,
  reviewWarning,
}: Props) {
  const [type, setType] = useState<ItemType>(initial?.type ?? 'flight');
  const [links, setLinks] = useState<Link[]>(initial?.links ?? []);
  const [selectedTravelers, setSelectedTravelers] = useState<Set<string>>(
    new Set(initial?.traveler_ids ?? []),
  );
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggleTraveler = (id: string) => {
    const next = new Set(selectedTravelers);
    next.has(id) ? next.delete(id) : next.add(id);
    setSelectedTravelers(next);
  };

  const addLinkRow = () => setLinks((ls) => [...ls, { label: '', url: '' }]);
  const updateLink = (i: number, patch: Partial<Link>) =>
    setLinks((ls) => ls.map((l, idx) => (idx === i ? { ...l, ...patch } : l)));
  const removeLink = (i: number) =>
    setLinks((ls) => ls.filter((_, idx) => idx !== i));

  const submit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setError(null);
    const form = e.currentTarget;
    const fd = new FormData(form);
    const title = String(fd.get('title') || '').trim();
    const start_at = String(fd.get('start_at') || '');
    const end_at_raw = String(fd.get('end_at') || '');
    const location = String(fd.get('location') || '').trim() || null;
    const confirmation_no =
      String(fd.get('confirmation_no') || '').trim() || null;
    const notes = String(fd.get('notes') || '');
    if (!title || !start_at) {
      setError('Title and start time are required.');
      return;
    }
    const filteredLinks = links.filter((l) => l.url.trim());
    const body: CreateItem = {
      type,
      title,
      start_at,
      end_at: end_at_raw || null,
      location,
      confirmation_no,
      notes,
      details: initial?.details ?? {},
      links: filteredLinks,
      traveler_ids: Array.from(selectedTravelers),
    };
    setSubmitting(true);
    try {
      await onSubmit(body);
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form
      onSubmit={submit}
      className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm"
    >
      {reviewWarning && (
        <p className="rounded-lg border border-amber-300 bg-amber-50 px-3 py-2 text-sm text-amber-900">
          <span className="font-medium">Needs review:</span> {reviewWarning}
        </p>
      )}

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {TYPES.map((t) => (
          <button
            type="button"
            key={t}
            onClick={() => setType(t)}
            className={`rounded-lg border px-3 py-2 text-sm font-medium ${
              type === t
                ? 'border-teal-600 bg-teal-50 text-teal-800'
                : 'border-slate-200 bg-white text-slate-700 hover:bg-slate-50'
            }`}
          >
            {TYPE_LABEL[t]}
          </button>
        ))}
      </div>

      <label className="block">
        <span className="text-sm font-medium text-slate-700">Title</span>
        <input
          name="title"
          required
          placeholder="JL043 SFO → HND"
          defaultValue={initial?.title ?? ''}
        />
      </label>

      <div className="grid grid-cols-2 gap-3">
        <label className="block">
          <span className="text-sm font-medium text-slate-700">Start</span>
          <input
            name="start_at"
            type="datetime-local"
            required
            defaultValue={toLocalInput(initial?.start_at)}
          />
        </label>
        <label className="block">
          <span className="text-sm font-medium text-slate-700">
            End <span className="text-slate-400">(optional)</span>
          </span>
          <input
            name="end_at"
            type="datetime-local"
            defaultValue={toLocalInput(initial?.end_at)}
          />
        </label>
      </div>

      <label className="block">
        <span className="text-sm font-medium text-slate-700">Location</span>
        <input
          name="location"
          placeholder="SFO Terminal I"
          defaultValue={initial?.location ?? ''}
        />
      </label>

      <label className="block">
        <span className="text-sm font-medium text-slate-700">
          Confirmation #
        </span>
        <input
          name="confirmation_no"
          placeholder="ABC123"
          defaultValue={initial?.confirmation_no ?? ''}
        />
      </label>

      <fieldset className="space-y-1">
        <legend className="text-sm font-medium text-slate-700">Links</legend>
        {links.map((l, i) => (
          <div key={i} className="grid grid-cols-[1fr_2fr_auto] gap-2">
            <input
              placeholder="Label"
              value={l.label}
              onChange={(e) => updateLink(i, { label: e.target.value })}
            />
            <input
              type="url"
              placeholder="https://…"
              value={l.url}
              onChange={(e) => updateLink(i, { url: e.target.value })}
            />
            <button
              type="button"
              onClick={() => removeLink(i)}
              className="rounded-lg px-2 text-sm text-slate-500 hover:text-red-700"
              aria-label="Remove link"
            >
              ✕
            </button>
          </div>
        ))}
        <button
          type="button"
          onClick={addLinkRow}
          className="text-xs text-teal-700 hover:underline"
        >
          + add link
        </button>
      </fieldset>

      {travelers.length > 0 && (
        <fieldset>
          <legend className="text-sm font-medium text-slate-700">
            Travelers
          </legend>
          <div className="mt-1 flex flex-wrap gap-1.5">
            {travelers.map((t) => (
              <button
                type="button"
                key={t.id}
                onClick={() => toggleTraveler(t.id)}
                className={`rounded-full px-2.5 py-1 text-xs font-medium ${
                  selectedTravelers.has(t.id)
                    ? 'bg-teal-600 text-white'
                    : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
                }`}
              >
                {t.name}
              </button>
            ))}
          </div>
        </fieldset>
      )}

      <label className="block">
        <span className="text-sm font-medium text-slate-700">Notes</span>
        <textarea name="notes" rows={2} defaultValue={initial?.notes ?? ''} />
      </label>

      {error && (
        <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      <div className="flex gap-2">
        <button
          type="submit"
          disabled={submitting}
          className="rounded-lg bg-teal-600 px-4 py-2 text-sm font-medium text-white hover:bg-teal-700 disabled:opacity-60"
        >
          {submitting ? 'Saving…' : 'Save'}
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
