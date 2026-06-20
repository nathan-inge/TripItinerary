export type ItemType = 'flight' | 'lodging' | 'transport' | 'activity';
export type ItemSource = 'paste' | 'email' | 'manual';
export type ParseStatus = 'parsed' | 'review' | 'manual';

export interface Trip {
  id: string;
  owner_email: string;
  name: string;
  destination: string | null;
  start_date: string;
  end_date: string;
  notes_md: string;
  created_at: string;
  updated_at: string;
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

export interface TripDetail extends Trip {
  travelers: Traveler[];
  items: Item[];
  day_notes: DayNote[];
}

export interface CreateTrip {
  name: string;
  destination?: string | null;
  start_date: string;
  end_date: string;
  notes_md?: string;
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
