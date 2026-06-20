import type { DayNote, Item, Traveler, TripDetail } from '../api/types';
import DayHeader from './DayHeader';
import DayNotesEditor from './DayNotesEditor';
import ItemCard from './ItemCard';

interface Props {
  trip: TripDetail;
  items: Item[];
  dayNotes: DayNote[];
  onSaveDayNote: (date: string, notes_md: string) => void | Promise<void>;
  onDeleteItem: (item: Item) => void | Promise<void>;
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

interface LodgingSpan {
  item: Item;
  startDate: string;
  endDate: string; // exclusive
}

export default function Timeline({
  trip,
  items,
  dayNotes,
  onSaveDayNote,
  onDeleteItem,
  readOnly,
}: Props) {
  const days = eachDate(trip.start_date, trip.end_date);
  const travelers: Traveler[] = trip.travelers;

  const itemsByDay = new Map<string, Item[]>();
  const lodgingSpans: LodgingSpan[] = [];

  for (const item of items) {
    if (item.type === 'lodging') {
      const startDate = isoDate(item.start_at);
      const endDate = item.end_at ? isoDate(item.end_at) : startDate;
      lodgingSpans.push({ item, startDate, endDate });
    } else {
      const d = isoDate(item.start_at);
      const list = itemsByDay.get(d) ?? [];
      list.push(item);
      itemsByDay.set(d, list);
    }
  }

  const noteFor = (date: string) =>
    dayNotes.find((n) => n.date === date)?.notes_md ?? '';

  const coversNight = (s: LodgingSpan, date: string) =>
    s.endDate > s.startDate
      ? date >= s.startDate && date < s.endDate
      : date === s.startDate;

  return (
    <div className="space-y-6">
      {days.map((date) => {
        const lodgings = lodgingSpans.filter((s) => coversNight(s, date));
        const checkoutsToday = lodgingSpans.filter(
          (s) => s.endDate === date && s.endDate > s.startDate,
        );
        const dayItems = itemsByDay.get(date) ?? [];

        return (
          <section key={date} className="space-y-2">
            <DayHeader date={date} />

            <DayNotesEditor
              initialValue={noteFor(date)}
              onSave={(v) => onSaveDayNote(date, v)}
              readOnly={readOnly}
            />

            {lodgings.map(({ item, startDate }) => (
              <div
                key={`stay-${item.id}-${date}`}
                className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900"
              >
                <span aria-hidden>🛏 </span>
                <span className="font-medium">{item.title}</span>
                {item.location && (
                  <span className="text-amber-800/80"> · {item.location}</span>
                )}
                {startDate === date && (
                  <span className="ml-2 text-xs text-amber-800/70">check-in</span>
                )}
              </div>
            ))}

            {checkoutsToday.map(({ item }) => (
              <div
                key={`checkout-${item.id}-${date}`}
                className="rounded-lg border border-amber-200 bg-amber-50/60 px-3 py-2 text-sm text-amber-900"
              >
                <span aria-hidden>🛏 </span>
                <span className="font-medium">{item.title}</span>
                <span className="ml-2 text-xs text-amber-800/70">check-out</span>
              </div>
            ))}

            {dayItems.map((item) => (
              <ItemCard
                key={item.id}
                item={item}
                travelers={travelers}
                onDelete={() => onDeleteItem(item)}
                readOnly={readOnly}
              />
            ))}

            {dayItems.length === 0 &&
              lodgings.length === 0 &&
              checkoutsToday.length === 0 &&
              !noteFor(date) && (
                <p className="text-xs text-slate-400">No items.</p>
              )}
          </section>
        );
      })}
    </div>
  );
}
