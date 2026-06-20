interface Props {
  date: string; // YYYY-MM-DD
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

export default function DayHeader({ date }: Props) {
  const [yyyy, mm, dd] = date.split('-').map(Number);
  // Build at UTC noon to avoid tz rollover on display.
  const d = new Date(Date.UTC(yyyy, mm - 1, dd, 12));
  const label = `${WEEKDAY[d.getUTCDay()]}, ${MONTH[d.getUTCMonth()]} ${dd}`;

  return (
    <div className="flex items-baseline gap-2 border-b border-slate-200 pb-1">
      <h2 className="text-lg font-bold text-slate-900">{label}</h2>
      <span className="text-xs tabular-nums text-slate-400">{date}</span>
    </div>
  );
}
