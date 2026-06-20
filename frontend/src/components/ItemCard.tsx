import { useState } from 'react';
import type { CreateItem, Item, ItemType, Traveler } from '../api/types';
import ItemLinks from './ItemLinks';
import AddItemForm from './AddItemForm';

interface Props {
  item: Item;
  travelers: Traveler[];
  onDelete: () => void;
  onUpdate?: (body: CreateItem) => void | Promise<void>;
  readOnly?: boolean;
}

const TYPE_LABEL: Record<ItemType, string> = {
  flight: '✈',
  lodging: '🛏',
  transport: '🚆',
  activity: '🎫',
};

const TYPE_COLOR: Record<ItemType, string> = {
  flight: 'bg-sky-50 text-sky-700 border-sky-200',
  lodging: 'bg-amber-50 text-amber-800 border-amber-200',
  transport: 'bg-violet-50 text-violet-700 border-violet-200',
  activity: 'bg-emerald-50 text-emerald-700 border-emerald-200',
};

function formatTime(iso: string | null): string {
  if (!iso) return '';
  // Preserve the local-to-item time as encoded in the offset.
  // Parse manually rather than via Date so we don't shift into the viewer's tz.
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(iso);
  if (!m) return iso;
  return `${m[4]}:${m[5]}`;
}

export default function ItemCard({
  item,
  travelers,
  onDelete,
  onUpdate,
  readOnly,
}: Props) {
  const [editing, setEditing] = useState(false);

  if (editing && onUpdate) {
    return (
      <AddItemForm
        travelers={travelers}
        initial={{
          type: item.type,
          title: item.title,
          start_at: item.start_at,
          end_at: item.end_at,
          location: item.location,
          confirmation_no: item.confirmation_no,
          notes: item.notes,
          details: item.details,
          links: item.links,
          traveler_ids: item.traveler_ids,
        }}
        onCancel={() => setEditing(false)}
        onSubmit={async (body) => {
          await onUpdate(body);
          setEditing(false);
        }}
      />
    );
  }

  const travelerNames = item.traveler_ids
    .map((tid) => travelers.find((t) => t.id === tid)?.name)
    .filter(Boolean) as string[];

  const canEdit = !readOnly && !!onUpdate;
  const openEdit = () => {
    if (canEdit) setEditing(true);
  };

  return (
    <article
      className={`rounded-xl border bg-white p-3 shadow-sm ${TYPE_COLOR[item.type]} ${
        canEdit ? 'cursor-pointer hover:shadow-md' : ''
      }`}
      role={canEdit ? 'button' : undefined}
      tabIndex={canEdit ? 0 : undefined}
      onClick={openEdit}
      onKeyDown={(e) => {
        if (canEdit && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault();
          openEdit();
        }
      }}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-base" aria-hidden>
              {TYPE_LABEL[item.type]}
            </span>
            <h3 className="truncate text-base font-semibold">{item.title}</h3>
          </div>
          <p className="mt-0.5 text-xs tabular-nums text-slate-600">
            {formatTime(item.start_at)}
            {item.end_at && <> – {formatTime(item.end_at)}</>}
            {item.location && (
              <span className="ml-2 text-slate-700">· {item.location}</span>
            )}
          </p>
          {travelerNames.length > 0 && (
            <p className="mt-1 text-xs text-slate-600">
              {travelerNames.join(', ')}
            </p>
          )}
          <div onClick={(e) => e.stopPropagation()}>
            <ItemLinks links={item.links} confirmationNo={item.confirmation_no} />
          </div>
          {item.notes && (
            <p className="mt-2 whitespace-pre-wrap text-sm text-slate-700">
              {item.notes}
            </p>
          )}
        </div>
        {!readOnly && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
            className="text-xs text-slate-400 hover:text-red-700"
            aria-label="Delete item"
            title="Delete"
          >
            ✕
          </button>
        )}
      </div>
    </article>
  );
}
