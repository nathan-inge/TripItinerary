import { useEffect, useState } from 'react';

interface Props {
  initialValue: string;
  onSave: (notes_md: string) => void | Promise<void>;
  readOnly?: boolean;
}

export default function DayNotesEditor({
  initialValue,
  onSave,
  readOnly,
}: Props) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(initialValue);

  useEffect(() => {
    setValue(initialValue);
  }, [initialValue]);

  if (readOnly) {
    if (!initialValue) return null;
    return (
      <div className="whitespace-pre-wrap rounded-lg bg-slate-50 px-3 py-2 text-sm text-slate-700">
        {initialValue}
      </div>
    );
  }

  if (editing) {
    return (
      <div className="space-y-2">
        <textarea
          value={value}
          onChange={(e) => setValue(e.target.value)}
          rows={3}
          placeholder="Free-form notes for this day…"
          className="font-sans text-sm"
        />
        <div className="flex gap-2">
          <button
            onClick={async () => {
              await onSave(value);
              setEditing(false);
            }}
            className="rounded-lg bg-teal-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-teal-700"
          >
            Save
          </button>
          <button
            onClick={() => {
              setValue(initialValue);
              setEditing(false);
            }}
            className="rounded-lg border border-slate-300 px-3 py-1.5 text-xs font-medium text-slate-600 hover:bg-slate-100"
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }

  if (!initialValue) {
    return (
      <button
        onClick={() => setEditing(true)}
        className="text-xs text-slate-400 hover:text-teal-700"
      >
        + add notes
      </button>
    );
  }

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => setEditing(true)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') setEditing(true);
      }}
      className="cursor-text whitespace-pre-wrap rounded-lg bg-slate-50 px-3 py-2 text-sm text-slate-700 hover:bg-slate-100"
    >
      {initialValue}
    </div>
  );
}
