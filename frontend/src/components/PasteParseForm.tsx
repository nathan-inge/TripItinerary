import { FormEvent, useState } from 'react';
import { api } from '../api/client';
import type { CreateItem, Traveler } from '../api/types';
import AddItemForm from './AddItemForm';

interface Props {
  tripId: string;
  travelers: Traveler[];
  onCancel: () => void;
  onSaved: () => void | Promise<void>;
}

type Preview = {
  initial: Partial<CreateItem>;
  reviewReason: string;
};

export default function PasteParseForm({
  tripId,
  travelers,
  onCancel,
  onSaved,
}: Props) {
  const [rawText, setRawText] = useState('');
  const [parsing, setParsing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);

  const parse = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setError(null);
    if (!rawText.trim()) return;
    setParsing(true);
    try {
      const resp = await api.parseText(tripId, rawText);
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
    await api.createItem(tripId, {
      ...body,
      // preserve the original paste alongside the (possibly edited) fields
      // so re-parse is always possible.
      // Backend stores raw_source for source='paste' items.
    });
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
