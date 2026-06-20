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
  onUpdateItem,
  readOnly,
}: Props) {
  const days = eachDate(trip.start_date, trip.end_date);
  const travelers: Traveler[] = trip.travelers;

  // Lodgings get split-rendered: an editable ItemCard on the check-in day
  // (so you can click to edit), and a smaller "stay continues" banner on
  // subsequent covered nights for orientation.
  const itemsByDay = new Map<string, Item[]>();
  const lodgingSpans: LodgingSpan[] = [];

  for (const item of items) {
    const d = isoDate(item.start_at);
    const list = itemsByDay.get(d) ?? [];
    list.push(item);
    itemsByDay.set(d, list);

    if (item.type === 'lodging') {
      const startDate = d;
      const endDate = item.end_at ? isoDate(item.end_at) : startDate;
      lodgingSpans.push({ item, startDate, endDate });
    }
  }

  const noteFor = (date: string) =>
    dayNotes.find((n) => n.date === date)?.notes_md ?? '';

  const isContinuingStay = (s: LodgingSpan, date: string) =>
    s.endDate > s.startDate && date > s.startDate && date < s.endDate;

  return (
    <div className="space-y-6">
      {days.map((date) => {
        const continuingStays = lodgingSpans.filter((s) =>
          isContinuingStay(s, date),
        );
        const checkoutsToday = lodgingSpans.filter(
          (s) => s.endDate === date && s.endDate > s.startDate,
        );
        const dayItems = (itemsByDay.get(date) ?? []).slice().sort((a, b) =>
          a.start_at.localeCompare(b.start_at),
        );

        return (
          <section key={date} className="space-y-2">
            <DayHeader date={date} />

            <DayNotesEditor
              initialValue={noteFor(date)}
              onSave={(v) => onSaveDayNote(date, v)}
              readOnly={readOnly}
            />

            {continuingStays.map(({ item }) => (
              <div
                key={`stay-${item.id}-${date}`}
                className="rounded-lg border border-amber-200 bg-amber-50/60 px-3 py-2 text-sm text-amber-900"
              >
                <span aria-hidden>🛏 </span>
                <span className="font-medium">{item.title}</span>
                {item.location && (
                  <span className="text-amber-800/80"> · {item.location}</span>
                )}
                <span className="ml-2 text-xs text-amber-800/70">stay continues</span>
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
                onUpdate={
                  onUpdateItem
                    ? (body) => onUpdateItem(item, body)
                    : undefined
                }
                readOnly={readOnly}
              />
            ))}

            {dayItems.length === 0 &&
              continuingStays.length === 0 &&
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
