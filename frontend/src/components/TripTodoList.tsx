import { FormEvent, useEffect, useRef, useState } from 'react';
import type { TodoItem } from '../api/types';

interface Props {
  todos: TodoItem[];
  onChange: (next: TodoItem[]) => void | Promise<void>;
  readOnly?: boolean;
}

function newId(): string {
  // Short, locally-unique enough; backend doesn't validate ids.
  return Math.random().toString(36).slice(2, 10);
}

export default function TripTodoList({ todos, onChange, readOnly }: Props) {
  const [draft, setDraft] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingText, setEditingText] = useState('');

  const add = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const text = draft.trim();
    if (!text) return;
    await onChange([...todos, { id: newId(), text, done: false }]);
    setDraft('');
  };

  const toggle = async (id: string) => {
    await onChange(
      todos.map((t) => (t.id === id ? { ...t, done: !t.done } : t)),
    );
  };

  const remove = async (id: string) => {
    await onChange(todos.filter((t) => t.id !== id));
  };

  const commitEdit = async (id: string) => {
    const text = editingText.trim();
    setEditingId(null);
    if (!text) {
      await onChange(todos.filter((t) => t.id !== id));
      return;
    }
    await onChange(todos.map((t) => (t.id === id ? { ...t, text } : t)));
  };

  if (readOnly && todos.length === 0) return null;

  // Display-only: open items first, completed sink to the bottom. Sort is
  // stable, so relative order within each group is preserved. Mutations key off
  // the original `todos` array by id, so the stored order is unaffected.
  const ordered = [...todos].sort((a, b) => Number(a.done) - Number(b.done));

  return (
    <section className="rounded-xl border border-slate-200 bg-white p-3 shadow-sm">
      <h2 className="text-sm font-semibold text-slate-700">To-do</h2>
      {todos.length === 0 ? (
        !readOnly && (
          <p className="mt-1 text-xs text-slate-400">
            Nothing yet. Add the first item below.
          </p>
        )
      ) : (
        <ul className="mt-2 space-y-1">
          {ordered.map((t) => (
            <TodoRow
              key={t.id}
              todo={t}
              readOnly={readOnly}
              editing={editingId === t.id}
              editingText={editingText}
              onStartEdit={() => {
                setEditingId(t.id);
                setEditingText(t.text);
              }}
              onChangeEdit={setEditingText}
              onCommitEdit={() => commitEdit(t.id)}
              onCancelEdit={() => setEditingId(null)}
              onToggle={() => toggle(t.id)}
              onDelete={() => remove(t.id)}
            />
          ))}
        </ul>
      )}
      {!readOnly && (
        <form onSubmit={add} className="mt-2 flex gap-2">
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="Add a to-do…"
            className="flex-1 text-sm"
          />
          <button
            type="submit"
            disabled={!draft.trim()}
            className="rounded-lg bg-teal-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-teal-700 disabled:opacity-50"
          >
            Add
          </button>
        </form>
      )}
    </section>
  );
}

interface RowProps {
  todo: TodoItem;
  readOnly?: boolean;
  editing: boolean;
  editingText: string;
  onStartEdit: () => void;
  onChangeEdit: (s: string) => void;
  onCommitEdit: () => void;
  onCancelEdit: () => void;
  onToggle: () => void;
  onDelete: () => void;
}

function TodoRow({
  todo,
  readOnly,
  editing,
  editingText,
  onStartEdit,
  onChangeEdit,
  onCommitEdit,
  onCancelEdit,
  onToggle,
  onDelete,
}: RowProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  return (
    <li className="group flex items-center gap-2 rounded-lg px-1 py-0.5 hover:bg-slate-50">
      <input
        type="checkbox"
        checked={todo.done}
        disabled={readOnly}
        onChange={onToggle}
        className="h-4 w-4 accent-teal-600"
        aria-label={todo.done ? 'Mark not done' : 'Mark done'}
      />
      {editing ? (
        <input
          ref={inputRef}
          value={editingText}
          onChange={(e) => onChangeEdit(e.target.value)}
          onBlur={onCommitEdit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              (e.target as HTMLInputElement).blur();
            } else if (e.key === 'Escape') {
              onCancelEdit();
            }
          }}
          className="flex-1 text-sm"
        />
      ) : (
        <span
          role={readOnly ? undefined : 'button'}
          tabIndex={readOnly ? undefined : 0}
          onClick={readOnly ? undefined : onStartEdit}
          onKeyDown={(e) => {
            if (!readOnly && (e.key === 'Enter' || e.key === ' ')) {
              e.preventDefault();
              onStartEdit();
            }
          }}
          className={`flex-1 cursor-text text-sm ${
            todo.done ? 'text-slate-400 line-through' : 'text-slate-700'
          }`}
        >
          {todo.text}
        </span>
      )}
      {!readOnly && (
        <button
          type="button"
          onClick={onDelete}
          aria-label="Delete to-do"
          title="Delete"
          className="text-xs text-slate-300 opacity-0 transition-opacity hover:text-red-700 group-hover:opacity-100 focus:opacity-100"
        >
          ✕
        </button>
      )}
    </li>
  );
}
