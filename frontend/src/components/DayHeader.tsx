interface Props {
  date: string; // YYYY-MM-DD
  expanded: boolean;
  onToggle: () => void;
  summary?: string; // shown when collapsed, e.g. "3 items"
  isToday?: boolean;
}

const WEEKDAY = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTH = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

export default function DayHeader({
  date,
  expanded,
  onToggle,
  summary,
  isToday,
}: Props) {
  const [yyyy, mm, dd] = date.split('-').map(Number);
  // Build at UTC noon to avoid tz rollover on display.
  const d = new Date(Date.UTC(yyyy, mm - 1, dd, 12));
  const label = `${WEEKDAY[d.getUTCDay()]}, ${MONTH[d.getUTCMonth()]} ${dd}`;

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={expanded}
      className="flex w-full items-baseline gap-2 border-b border-slate-200 pb-1 text-left"
    >
      <span
        aria-hidden
        className={`self-center text-xs text-slate-400 transition-transform ${
          expanded ? 'rotate-0' : '-rotate-90'
        }`}
      >
        ▾
      </span>
      <h2 className="text-lg font-bold text-slate-900">{label}</h2>
      {isToday && (
        <span className="rounded-full bg-emerald-100 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-emerald-700">
          Today
        </span>
      )}
      <span className="text-xs tabular-nums text-slate-400">{date}</span>
      {!expanded && summary && (
        <span className="ml-auto text-xs text-slate-400">{summary}</span>
      )}
    </button>
  );
}
