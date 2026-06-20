import type { Link } from '../api/types';

interface Props {
  links: Link[];
  confirmationNo: string | null;
}

export default function ItemLinks({ links, confirmationNo }: Props) {
  if (links.length === 0 && !confirmationNo) return null;
  return (
    <div className="mt-2 flex flex-wrap items-center gap-1.5">
      {links.map((l, i) => (
        <a
          key={`${l.url}-${i}`}
          href={l.url}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1 rounded-full bg-teal-50 px-2.5 py-1 text-xs font-medium text-teal-800 hover:bg-teal-100"
        >
          <span aria-hidden>↗</span>
          {l.label || 'Link'}
        </a>
      ))}
      {confirmationNo && (
        <button
          type="button"
          onClick={() => {
            navigator.clipboard?.writeText(confirmationNo);
          }}
          title="Copy confirmation number"
          className="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2.5 py-1 font-mono text-xs text-slate-700 hover:bg-slate-200"
        >
          <span aria-hidden>#</span>
          {confirmationNo}
        </button>
      )}
    </div>
  );
}
