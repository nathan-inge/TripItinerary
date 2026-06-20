-- Per-trip sender allow-list for the email-intake feature.
-- An incoming forwarded confirmation is accepted only if its `From:` matches
-- one of the rows for the trip identified by the trip-name hint in the body.

CREATE TABLE trip_email_senders (
    trip_id    TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
    email      TEXT NOT NULL,             -- stored lowercase, trimmed
    created_at TEXT NOT NULL,
    PRIMARY KEY (trip_id, email)
);

CREATE INDEX idx_trip_email_senders_email ON trip_email_senders(email);
