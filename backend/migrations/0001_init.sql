PRAGMA foreign_keys = ON;

CREATE TABLE trips (
    id          TEXT PRIMARY KEY,
    owner_email TEXT NOT NULL,
    name        TEXT NOT NULL,
    destination TEXT,
    start_date  TEXT NOT NULL,
    end_date    TEXT NOT NULL,
    notes_md    TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_trips_owner_email ON trips(owner_email);
CREATE INDEX idx_trips_dates       ON trips(start_date, end_date);

CREATE TABLE travelers (
    id         TEXT PRIMARY KEY,
    trip_id    TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_travelers_trip_id ON travelers(trip_id);

CREATE TABLE items (
    id              TEXT PRIMARY KEY,
    trip_id         TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
    type            TEXT NOT NULL CHECK (type IN ('flight','lodging','transport','activity')),
    title           TEXT NOT NULL,
    start_at        TEXT NOT NULL,
    end_at          TEXT,
    location        TEXT,
    confirmation_no TEXT,
    notes           TEXT NOT NULL DEFAULT '',
    details_json    TEXT NOT NULL DEFAULT '{}',
    links_json      TEXT NOT NULL DEFAULT '[]',
    raw_source      TEXT,
    source          TEXT NOT NULL CHECK (source IN ('paste','email','manual')) DEFAULT 'manual',
    parse_status    TEXT NOT NULL CHECK (parse_status IN ('parsed','review','manual')) DEFAULT 'manual',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX idx_items_trip_start ON items(trip_id, start_at);

CREATE TABLE item_travelers (
    item_id     TEXT NOT NULL REFERENCES items(id)     ON DELETE CASCADE,
    traveler_id TEXT NOT NULL REFERENCES travelers(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, traveler_id)
);

CREATE TABLE day_notes (
    trip_id    TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
    date       TEXT NOT NULL,
    notes_md   TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL,
    PRIMARY KEY (trip_id, date)
);

CREATE TABLE share_links (
    id           TEXT PRIMARY KEY,
    trip_id      TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
    label        TEXT,
    scope        TEXT NOT NULL CHECK (scope IN ('view','edit')),
    token_hash   TEXT NOT NULL UNIQUE,
    created_at   TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at   TEXT
);

CREATE INDEX idx_share_token_hash_active
    ON share_links(token_hash) WHERE revoked_at IS NULL;

CREATE TABLE intake_inbox (
    id               TEXT PRIMARY KEY,
    raw_text         TEXT NOT NULL,
    from_addr        TEXT,
    subject          TEXT,
    received_at      TEXT NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('pending','assigned','discarded')) DEFAULT 'pending',
    assigned_item_id TEXT REFERENCES items(id) ON DELETE SET NULL
);

CREATE INDEX idx_intake_inbox_status ON intake_inbox(status);
