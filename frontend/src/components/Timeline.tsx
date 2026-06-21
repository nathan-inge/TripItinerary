import type {
  CreateItem,
  DayNote,
  Item,
  Traveler,
  TripDetail,
} from '../api/types';
import DayHeader from './DayHeader';
import DayNotesEditor from './DayNotesEditor';
import ItemCard from './ItemCard';

interface Props {
  trip: TripDetail;
  items: Item[];
  dayNotes: DayNote[];
  onSaveDayNote: (date: string, notes_md: string) => void | Promise<void>;
  onDeleteItem: (item: Item) => void | Promise<void>;
  onUpdateItem?: (item: Item, body: CreateItem) => void | Promise<void>;
  readOnly?: boolean;
}

function isoDate(iso: string): string {
  // First 10 chars of an RFC3339 string are the local-to-item YYYY-MM-DD.
  return iso.slice(0, 10);
}

function eachDate(start: string, end: string): string[] {
  const out: string[] = [];
  const [sy, sm, sd] = start.split('-').map(Number);
  const [ey, em, ed] = end.split('-').map(Number);
  const cur = new Date(Date.UTC(sy, sm - 1, sd));
  const stop = new Date(Date.UTC(ey, em - 1, ed));
  while (cur <= stop) {
    out.push(cur.toISOString().slice(0, 10));
    cur.setUTCDate(cur.getUTCDate() + 1);
  }
  return out;
}

// A multi-day "held" item rendered with continuation banners on each
// intermediate day and an end-day banner. Lodging (any duration ≥ 0) and
// transport with endDate > startDate qualify. Flights are deliberately
// excluded — you're not "still on the flight" the next day, you've arrived.
interface Span {
  item: Item;
  kind: 'lodging' | 'transport';
  startDate: string;
  endDate: string;
}

const SPAN_STYLE: Record<Span['kind'], { icon: string; box: string; end: string }> = {
  lodging: {
    icon: '🛏',
    box: 'border-amber-200 bg-amber-50/60 text-amber-900',
    end: 'check-out',
  },
  transport: {
    icon: '🚆',
    box: 'border-violet-200 bg-violet-50/60 text-violet-900',
    end: 'drop-off',
  },
};

function spanFor(item: Item): Span | null {
  const startDate = isoDate(item.start_at);
  const endDate = item.end_at ? isoDate(item.end_at) : startDate;
  if (item.type === 'lodging') {
    return { item, kind: 'lodging', startDate, endDate };
  }
  if (item.type === 'transport' && endDate > startDate) {
    return { item, kind: 'transport', startDate, endDate };
  }
  return null;
}

export default function Timeline({
  trip,
  items,
  dayNotes,
  onSaveDayNote,
  onDeleteItem,
  onUpdateItem,
  readOnly,
}: Props) {
  const days = eachDate(trip.start_date, trip.end_date);
  const travelers: Traveler[] = trip.travelers;

  const itemsByDay = new Map<string, Item[]>();
  const spans: Span[] = [];

  for (const item of items) {
    const d = isoDate(item.start_at);
    const list = itemsByDay.get(d) ?? [];
    list.push(item);
    itemsByDay.set(d, list);

    const s = spanFor(item);
    if (s) spans.push(s);
  }

  const noteFor = (date: string) =>
    dayNotes.find((n) => n.date === date)?.notes_md ?? '';

  const isContinuing = (s: Span, date: string) =>
    s.endDate > s.startDate && date > s.startDate && date < s.endDate;

  return (
    <div className="space-y-6">
      {days.map((date) => {
        const continuing = spans.filter((s) => isContinuing(s, date));
        const endsToday = spans.filter(
          (s) => s.endDate === date && s.endDate > s.startDate,
        );
        const dayItems = (itemsByDay.get(date) ?? [])
          .slice()
          .sort((a, b) => a.start_at.localeCompare(b.start_at));

        return (
          <section key={date} className="space-y-2">
            <DayHeader date={date} />

            <DayNotesEditor
              initialValue={noteFor(date)}
              onSave={(v) => onSaveDayNote(date, v)}
              readOnly={readOnly}
            />

            {continuing.map((s) => (
              <SpanBanner
                key={`cont-${s.item.id}-${date}`}
                span={s}
                label={s.kind === 'lodging' ? 'stay continues' : 'rental continues'}
              />
            ))}

            {endsToday.map((s) => (
              <SpanBanner
                key={`end-${s.item.id}-${date}`}
                span={s}
                label={SPAN_STYLE[s.kind].end}
              />
            ))}

            {dayItems.map((item) => (
              <ItemCard
                key={item.id}
                item={item}
                travelers={travelers}
                onDelete={() => onDeleteItem(item)}
                onUpdate={
                  onUpdateItem
                    ? (body) => onUpdateItem(item, body)
                    : undefined
                }
                readOnly={readOnly}
              />
            ))}

            {dayItems.length === 0 &&
              continuing.length === 0 &&
              endsToday.length === 0 &&
              !noteFor(date) && (
                <p className="text-xs text-slate-400">No items.</p>
              )}
          </section>
        );
      })}
    </div>
  );
}

function SpanBanner({ span, label }: { span: Span; label: string }) {
  const style = SPAN_STYLE[span.kind];
  const { item } = span;
  return (
    <div
      className={`rounded-lg border px-3 py-2 text-sm ${style.box}`}
    >
      <span aria-hidden>{style.icon} </span>
      <span className="font-medium">{item.title}</span>
      {item.location && (
        <>
          <span className="opacity-80"> · </span>
          <a
            href={`https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(item.location)}`}
            target="_blank"
            rel="noopener noreferrer"
            className="underline decoration-current/30 underline-offset-2 opacity-80 hover:decoration-current/70"
          >
            {item.location}
          </a>
        </>
      )}
      <span className="ml-2 text-xs opacity-70">{label}</span>
    </div>
  );
}

