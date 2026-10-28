export type ItemType = 'flight' | 'lodging' | 'transport' | 'activity';
export type ItemSource = 'paste' | 'email' | 'manual';
export type ParseStatus = 'parsed' | 'review' | 'manual';

export interface TodoItem {
  id: string;
  text: string;
  done: boolean;
}

export interface Me {
  email: string;
  /** False when signed in through Cloudflare Access but with no access in the
   *  app yet — nothing has been shared with this address. */
  provisioned: boolean;
}

export interface Trip {
  id: string;
  owner_email: string;
  name: string;
  destination: string | null;
  start_date: string;
  end_date: string;
  notes_md: string;
  photos_url: string | null;
  maps_url: string | null;
  todos: TodoItem[];
  created_at: string;
  updated_at: string;
}

/** The caller's access to a trip. `owner` holds the trip; `edit`/`view` come
 *  from an email-share membership (or, for guests, the share-link scope). */
export type TripRole = 'owner' | 'edit' | 'view';

/** A row of GET /trips — a trip plus the caller's role on it. */
export interface TripSummary extends Trip {
  my_role: TripRole;
}

export interface TripMember {
  trip_id: string;
  email: string;
  role: ShareScope;
  created_at: string;
}

export interface UpsertTripMember {
  email: string;
  role: ShareScope;
}

export interface Traveler {
  id: string;
  trip_id: string;
  name: string;
  sort_order: number;
}

export interface Link {
  label: string;
  url: string;
}

export interface Item {
  id: string;
  trip_id: string;
  type: ItemType;
  title: string;
  start_at: string;
  end_at: string | null;
  location: string | null;
  confirmation_no: string | null;
  notes: string;
  details: Record<string, unknown>;
  links: Link[];
  traveler_ids: string[];
  raw_source: string | null;
  source: ItemSource;
  parse_status: ParseStatus;
  created_at: string;
  updated_at: string;
}

export interface DayNote {
  trip_id: string;
  date: string;
  notes_md: string;
  updated_at: string;
}

export interface TripEmailSender {
  trip_id: string;
  email: string;
  created_at: string;
}

export interface TripDetail extends Trip {
  travelers: Traveler[];
  items: Item[];
  day_notes: DayNote[];
  /** Owner-only config; empty for members and share-link guests. */
  email_senders: TripEmailSender[];
  my_role: TripRole;
}

export interface CreateTrip {
  name: string;
  destination?: string | null;
  start_date: string;
  end_date: string;
  notes_md?: string;
}

export interface UpdateTrip {
  name?: string;
  destination?: string | null;
  start_date?: string;
  end_date?: string;
  notes_md?: string;
  photos_url?: string | null;
  maps_url?: string | null;
  todos?: TodoItem[];
}

export interface CreateItem {
  type: ItemType;
  title: string;
  start_at: string;
  end_at?: string | null;
  location?: string | null;
  confirmation_no?: string | null;
  notes?: string;
  details?: Record<string, unknown>;
  links?: Link[];
  traveler_ids?: string[];
}

export interface ItemPreview {
  type: ItemType;
  title: string;
  start_at: string;
  end_at: string | null;
  location: string | null;
  confirmation_no: string | null;
  notes: string;
  details: Record<string, unknown>;
  links: Link[];
  raw_source: string;
  source: ItemSource;
  parse_status: ParseStatus;
}

export interface ParseTextResponse {
  preview: ItemPreview;
  review_reason: string;
}

export type ShareScope = 'view' | 'edit';

export interface ShareLink {
  id: string;
  trip_id: string;
  label: string | null;
  scope: ShareScope;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

export interface CreatedShareLink extends ShareLink {
  token: string;
  url_path: string;
}

export interface CreateShareLink {
  label?: string | null;
  scope: ShareScope;
}

export interface SharedTripDetail extends TripDetail {
  share_scope: ShareScope;
}
