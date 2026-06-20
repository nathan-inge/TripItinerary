import type {
  CreateItem,
  CreateShareLink,
  CreateTrip,
  CreatedShareLink,
  DayNote,
  Item,
  ParseTextResponse,
  ShareLink,
  SharedTripDetail,
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
  parseText: (tripId: string, raw_text: string) =>
    req<ParseTextResponse>(`/trips/${tripId}/items/parse`, {
      method: 'POST',
      body: JSON.stringify({ raw_text }),
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

  listShareLinks: (tripId: string) =>
    req<ShareLink[]>(`/trips/${tripId}/share`),
  createShareLink: (tripId: string, body: CreateShareLink) =>
    req<CreatedShareLink>(`/trips/${tripId}/share`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  revokeShareLink: (id: string) =>
    req<void>(`/share-links/${id}`, { method: 'DELETE' }, false),

  addEmailSender: (tripId: string, email: string) =>
    req<{ trip_id: string; email: string; created_at: string }>(
      `/trips/${tripId}/email-senders`,
      { method: 'POST', body: JSON.stringify({ email }) },
    ),
  removeEmailSender: (tripId: string, email: string) =>
    req<void>(
      `/trips/${tripId}/email-senders/${encodeURIComponent(email)}`,
      { method: 'DELETE' },
      false,
    ),
};

// Returns a token-scoped API for the guest /share/:token page.
export function shareApi(token: string) {
  const base = `/share/${encodeURIComponent(token)}`;
  return {
    getTrip: () => req<SharedTripDetail>(`${base}/trip`),
    createItem: (body: CreateItem) =>
      req<Item>(`${base}/items`, {
        method: 'POST',
        body: JSON.stringify(body),
      }),
    updateItem: (id: string, body: Partial<CreateItem>) =>
      req<Item>(`${base}/items/${id}`, {
        method: 'PATCH',
        body: JSON.stringify(body),
      }),
    deleteItem: (id: string) =>
      req<void>(`${base}/items/${id}`, { method: 'DELETE' }, false),
    upsertDayNote: (date: string, notes_md: string) =>
      req<DayNote>(`${base}/day-notes/${date}`, {
        method: 'PUT',
        body: JSON.stringify({ notes_md }),
      }),
    parseText: (raw_text: string) =>
      req<ParseTextResponse>(`${base}/items/parse`, {
        method: 'POST',
        body: JSON.stringify({ raw_text }),
      }),
  };
}
