import type {
  CreateItem,
  CreateTrip,
  DayNote,
  Item,
  Traveler,
  Trip,
  TripDetail,
} from './types';

async function req<T>(
  path: string,
  init: RequestInit = {},
  parseJson = true,
): Promise<T> {
  const resp = await fetch(`/api${path}`, {
    headers: { 'content-type': 'application/json' },
    ...init,
  });
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`${resp.status} ${resp.statusText}: ${text}`);
  }
  if (!parseJson) return undefined as unknown as T;
  return (await resp.json()) as T;
}

export const api = {
  listTrips: () => req<Trip[]>('/trips'),
  getTrip: (id: string) => req<TripDetail>(`/trips/${id}`),
  createTrip: (body: CreateTrip) =>
    req<Trip>('/trips', { method: 'POST', body: JSON.stringify(body) }),
  updateTrip: (id: string, body: Partial<CreateTrip>) =>
    req<Trip>(`/trips/${id}`, { method: 'PATCH', body: JSON.stringify(body) }),
  deleteTrip: (id: string) =>
    req<void>(`/trips/${id}`, { method: 'DELETE' }, false),

  createTraveler: (tripId: string, name: string) =>
    req<Traveler>(`/trips/${tripId}/travelers`, {
      method: 'POST',
      body: JSON.stringify({ name }),
    }),
  deleteTraveler: (id: string) =>
    req<void>(`/travelers/${id}`, { method: 'DELETE' }, false),

  createItem: (tripId: string, body: CreateItem) =>
    req<Item>(`/trips/${tripId}/items`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateItem: (id: string, body: Partial<CreateItem>) =>
    req<Item>(`/items/${id}`, { method: 'PATCH', body: JSON.stringify(body) }),
  deleteItem: (id: string) =>
    req<void>(`/items/${id}`, { method: 'DELETE' }, false),

  upsertDayNote: (tripId: string, date: string, notes_md: string) =>
    req<DayNote>(`/trips/${tripId}/day-notes/${date}`, {
      method: 'PUT',
      body: JSON.stringify({ notes_md }),
    }),
};
