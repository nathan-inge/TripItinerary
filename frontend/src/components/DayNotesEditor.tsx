import { useEffect, useMemo, useState } from 'react';
import { EditorContent, useEditor, type Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Highlight from '@tiptap/extension-highlight';
import Placeholder from '@tiptap/extension-placeholder';
import DOMPurify, { type Config as DOMPurifyConfig } from 'dompurify';

interface Props {
  initialValue: string;
  onSave: (notes_md: string) => void | Promise<void>;
  readOnly?: boolean;
}

/** Allow-list for DOMPurify when rendering rich notes. */
const SANITIZE_CFG: DOMPurifyConfig = {
  RETURN_TRUSTED_TYPE: false,
  ALLOWED_TAGS: [
    'p',
    'br',
    'strong',
    'em',
    's',
    'u',
    'mark',
    'a',
    'h1',
    'h2',
    'h3',
    'ul',
    'ol',
    'li',
    'code',
    'pre',
    'blockquote',
  ],
  ALLOWED_ATTR: ['href', 'target', 'rel'],
  // Prevent javascript: / data: URLs in links.
  ALLOWED_URI_REGEXP:
    /^(?:(?:https?|mailto|tel|geo|sms):|[#/?]|[a-z0-9_+.\-]+@)/i,
};

/**
 * Existing notes were stored as plain text. If the value has no HTML tags,
 * wrap it into a TipTap-friendly paragraph (preserving newlines as <br>).
 */
function asRichHtml(s: string): string {
  if (!s) return '';
  const hasTag = /<[a-z][\s\S]*>/i.test(s);
  if (hasTag) return s;
  const escaped = s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
  const paragraphs = escaped
    .split(/\n{2,}/)
    .map((p) => `<p>${p.replace(/\n/g, '<br/>')}</p>`)
    .join('');
  return paragraphs;
}

function isEmptyHtml(html: string): boolean {
  if (!html) return true;
  const stripped = html
    .replace(/<\/?p[^>]*>/gi, '')
    .replace(/<br\s*\/?>(?:\s)*/gi, '')
    .replace(/&nbsp;/gi, '')
    .trim();
  return stripped === '';
}

export default function DayNotesEditor({
  initialValue,
  onSave,
  readOnly,
}: Props) {
  const [editing, setEditing] = useState(false);

  if (readOnly || !editing) {
    return (
      <RenderedNote
        html={initialValue}
        onClick={readOnly ? undefined : () => setEditing(true)}
        readOnly={!!readOnly}
      />
    );
  }

  return (
    <RichEditor
      initial={initialValue}
      onCancel={() => setEditing(false)}
      onSave={async (html) => {
        await onSave(html);
        setEditing(false);
      }}
    />
  );
}

function RenderedNote({
  html,
  onClick,
  readOnly,
}: {
  html: string;
  onClick?: () => void;
  readOnly: boolean;
}) {
  const safe = useMemo(
    () => DOMPurify.sanitize(asRichHtml(html), SANITIZE_CFG),
    [html],
  );

  if (!html) {
    if (readOnly) return null;
    return (
      <button
        onClick={onClick}
        className="text-xs text-slate-400 hover:text-teal-700"
      >
        + add notes
      </button>
    );
  }

  if (readOnly) {
    return (
      <div
        className="day-note-prose rounded-lg bg-slate-50 px-3 py-2"
        dangerouslySetInnerHTML={{ __html: safe }}
      />
    );
  }

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onClick?.();
        }
      }}
      className="day-note-prose cursor-text rounded-lg bg-slate-50 px-3 py-2 hover:bg-slate-100"
      dangerouslySetInnerHTML={{ __html: safe }}
    />
  );
}

function RichEditor({
  initial,
  onSave,
  onCancel,
}: {
  initial: string;
  onSave: (html: string) => void | Promise<void>;
  onCancel: () => void;
}) {
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
      }),
      Link.configure({
        openOnClick: false,
        autolink: true,
        HTMLAttributes: { rel: 'noopener noreferrer', target: '_blank' },
      }),
      Highlight,
      Placeholder.configure({
        placeholder: 'Free-form notes for this day…',
      }),
    ],
    content: asRichHtml(initial),
    editorProps: {
      attributes: {
        class: 'day-note-prose',
      },
    },
  });

  if (!editor) return null;

  const save = async () => {
    const html = editor.getHTML();
    const out = isEmptyHtml(html) ? '' : DOMPurify.sanitize(html, SANITIZE_CFG);
    await onSave(out);
  };

  return (
    <div className="space-y-2">
      <Toolbar editor={editor} />
      <EditorContent editor={editor} />
      <div className="flex gap-2">
        <button
          onClick={save}
          className="rounded-lg bg-teal-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-teal-700"
        >
          Save
        </button>
        <button
          onClick={onCancel}
          className="rounded-lg border border-slate-300 px-3 py-1.5 text-xs font-medium text-slate-600 hover:bg-slate-100"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}

function Toolbar({ editor }: { editor: Editor }) {
  // Force a rerender on editor state change so active states stay in sync.
  const [, force] = useState(0);
  useEffect(() => {
    const fn = () => force((n) => n + 1);
    editor.on('selectionUpdate', fn);
    editor.on('transaction', fn);
    return () => {
      editor.off('selectionUpdate', fn);
      editor.off('transaction', fn);
    };
  }, [editor]);

  const btn = (active: boolean, disabled: boolean) =>
    `rounded px-1.5 py-0.5 text-xs ${
      active ? 'bg-slate-800 text-white' : 'text-slate-600 hover:bg-slate-200'
    } ${disabled ? 'cursor-not-allowed opacity-40' : ''}`;

  const promptLink = () => {
    const prev = editor.getAttributes('link').href as string | undefined;
    const next = window.prompt('Link URL (leave blank to remove)', prev ?? '');
    if (next === null) return;
    const trimmed = next.trim();
    if (!trimmed) {
      editor.chain().focus().extendMarkRange('link').unsetLink().run();
      return;
    }
    let normalized = trimmed;
    if (!/^[a-z]+:|^[#/?]/i.test(normalized)) normalized = `https://${normalized}`;
    editor
      .chain()
      .focus()
      .extendMarkRange('link')
      .setLink({ href: normalized })
      .run();
  };

  return (
    <div className="flex flex-wrap gap-1 rounded-lg border border-slate-200 bg-slate-50 px-2 py-1">
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleBold().run()}
        className={btn(editor.isActive('bold'), false)}
        aria-label="Bold"
        title="Bold (⌘B)"
      >
        <strong>B</strong>
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleItalic().run()}
        className={btn(editor.isActive('italic'), false)}
        aria-label="Italic"
        title="Italic (⌘I)"
      >
        <em>I</em>
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleStrike().run()}
        className={btn(editor.isActive('strike'), false)}
        aria-label="Strikethrough"
        title="Strikethrough"
      >
        <s>S</s>
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleHighlight().run()}
        className={btn(editor.isActive('highlight'), false)}
        aria-label="Highlight"
        title="Highlight"
      >
        <span className="rounded bg-yellow-200 px-1">H</span>
      </button>
      <span className="mx-1 w-px bg-slate-300" aria-hidden />
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        className={btn(editor.isActive('heading', { level: 2 }), false)}
        title="Heading"
      >
        H2
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleBulletList().run()}
        className={btn(editor.isActive('bulletList'), false)}
        title="Bullet list"
      >
        •
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
        className={btn(editor.isActive('orderedList'), false)}
        title="Numbered list"
      >
        1.
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().sinkListItem('listItem').run()}
        disabled={!editor.can().sinkListItem('listItem')}
        className={btn(false, !editor.can().sinkListItem('listItem'))}
        title="Indent list item (Tab)"
      >
        →
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().liftListItem('listItem').run()}
        disabled={!editor.can().liftListItem('listItem')}
        className={btn(false, !editor.can().liftListItem('listItem'))}
        title="Outdent list item (Shift+Tab)"
      >
        ←
      </button>
      <span className="mx-1 w-px bg-slate-300" aria-hidden />
      <button
        type="button"
        onClick={promptLink}
        className={btn(editor.isActive('link'), false)}
        title="Link"
      >
        🔗
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
        className={btn(editor.isActive('blockquote'), false)}
        title="Quote"
      >
        ❝
      </button>
      <span className="mx-1 w-px bg-slate-300" aria-hidden />
      <button
        type="button"
        onClick={() => editor.chain().focus().undo().run()}
        disabled={!editor.can().undo()}
        className={btn(false, !editor.can().undo())}
        title="Undo (⌘Z)"
      >
        ↶
      </button>
      <button
        type="button"
        onClick={() => editor.chain().focus().redo().run()}
        disabled={!editor.can().redo()}
        className={btn(false, !editor.can().redo())}
        title="Redo (⌘⇧Z)"
      >
        ↷
      </button>
    </div>
  );
}
