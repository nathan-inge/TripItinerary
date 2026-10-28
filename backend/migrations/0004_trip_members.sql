-- Email-based sharing. A member is an address that authenticates through
-- Cloudflare Access with its own identity (as opposed to a share-link guest,
-- who is anonymous and bearer-authed). Members see the trip on their own home
-- page; `role` mirrors the share-link scopes.
--
-- Note: membership is the app-level grant. The address must *also* be on the
-- Cloudflare Access policy for the hostname, or it never reaches the app.

CREATE TABLE trip_members (
    trip_id    TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
    email      TEXT NOT NULL,              -- stored lowercase, trimmed
    role       TEXT NOT NULL CHECK (role IN ('view','edit')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (trip_id, email)
);

-- Drives the "trips shared with me" query and the provisioned-user check.
CREATE INDEX idx_trip_members_email ON trip_members(email);
