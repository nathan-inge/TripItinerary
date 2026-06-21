import { useEffect, useState } from 'react';
import type { UpdateTrip } from '../api/types';

interface Props {
  name: string;
  startDate: string;
  endDate: string;
  destination: string | null;
  photosUrl: string | null;
  mapsUrl: string | null;
  onPatch: (patch: UpdateTrip) => void | Promise<void>;
}

function isHttpUrl(s: string): boolean {
  try {
    const u = new URL(s);
    return u.protocol === 'http:' || u.protocol === 'https:';
  } catch {
    return false;
  }
}

export default function TripHeader({
  name,
  startDate,
  endDate,
  destination,
  photosUrl,
  mapsUrl,
  onPatch,
}: Props) {
  return (
    <div className="space-y-2">
      <EditableName value={name} onSave={(v) => onPatch({ name: v })} />
      <p className="text-sm text-slate-600">
        {startDate} → {endDate}
        {destination && <> · {destination}</>}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <UrlPill
          label="Photos"
          icon="📷"
          url={photosUrl}
          onSave={(v) => onPatch({ photos_url: v })}
        />
        <UrlPill
          label="Map"
          icon="🗺"
          url={mapsUrl}
          onSave={(v) => onPatch({ maps_url: v })}
        />
      </div>
    </div>
  );
}

function EditableName({
  value,
  onSave,
}: {
  value: string;
  onSave: (next: string) => void | Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  if (!editing) {
    return (
      <h1
        role="button"
        tabIndex={0}
        onClick={() => setEditing(true)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setEditing(true);
          }
        }}
        title="Click to rename"
        className="group inline-flex cursor-text items-center gap-2 text-2xl font-bold"
      >
        {value}
        <span
          aria-hidden
          className="text-sm font-normal text-slate-300 group-hover:text-slate-500"
        >
          ✎
        </span>
      </h1>
    );
  }

  const commit = async () => {
    const next = draft.trim();
    if (!next || next === value) {
      setDraft(value);
      setEditing(false);
      return;
    }
    await onSave(next);
    setEditing(false);
  };

  return (
    <input
      autoFocus
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          (e.target as HTMLInputElement).blur();
        } else if (e.key === 'Escape') {
          setDraft(value);
          setEditing(false);
        }
      }}
      className="w-full text-2xl font-bold"
    />
  );
}

function UrlPill({
  label,
  icon,
  url,
  onSave,
}: {
  label: string;
  icon: string;
  url: string | null;
  onSave: (next: string | null) => void | Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(url ?? '');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(url ?? '');
  }, [url]);

  if (editing) {
    const commit = async () => {
      const v = draft.trim();
      if (v && !isHttpUrl(v)) {
        setError('Must be an http(s) URL');
        return;
      }
      await onSave(v || null);
      setError(null);
      setEditing(false);
    };
    return (
      <span className="inline-flex items-center gap-1">
        <input
          autoFocus
          value={draft}
          placeholder={`${label} URL`}
          onChange={(e) => {
            setDraft(e.target.value);
            setError(null);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              commit();
            } else if (e.key === 'Escape') {
              setDraft(url ?? '');
              setError(null);
              setEditing(false);
            }
          }}
          className="rounded-full border border-slate-300 px-3 py-1 text-xs"
        />
        <button
          type="button"
          onClick={commit}
          className="rounded-full bg-teal-600 px-2 py-1 text-xs font-medium text-white hover:bg-teal-700"
        >
          Save
        </button>
        <button
          type="button"
          onClick={() => {
            setDraft(url ?? '');
            setError(null);
            setEditing(false);
          }}
          className="text-xs text-slate-500 hover:text-slate-700"
        >
          Cancel
        </button>
        {error && (
          <span className="text-xs text-red-700" role="alert">
            {error}
          </span>
        )}
      </span>
    );
  }

  if (!url) {
    return (
      <button
        type="button"
        onClick={() => setEditing(true)}
        className="inline-flex items-center gap-1 rounded-full border border-dashed border-slate-300 px-2.5 py-1 text-xs text-slate-500 hover:bg-slate-50"
      >
        + {label}
      </button>
    );
  }

  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-teal-50 pl-2.5 pr-1 text-xs font-medium text-teal-800">
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1 py-1 hover:underline"
      >
        <span aria-hidden>{icon}</span>
        {label}
      </a>
      <button
        type="button"
        onClick={() => setEditing(true)}
        title={`Change ${label} URL`}
        aria-label={`Change ${label} URL`}
        className="rounded-full px-1 text-teal-700/60 hover:text-teal-900"
      >
        ✎
      </button>
    </span>
  );
}
