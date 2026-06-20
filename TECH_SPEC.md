# TripItinerary — Technical Implementation Spec

Companion to [REQUIREMENTS.md](./REQUIREMENTS.md). Mirrors the
ContextualTaskOrganizer deploy shape so the same CloudDesktop can run both.

## 1. Architecture

One Rust binary, one SQLite file, one Cloudflare Tunnel. The SPA is built and
embedded into the binary via `rust-embed`, same as CTO.

```
                Cloudflare Tunnel
 user / guests ───────────────────►  travel.nathaninge.com
                                          │
                  ┌───────────────────────┴───────────────────────┐
                  │  Cloudflare Access policies on this hostname  │
                  │  - /share/*    → Bypass (public)              │
                  │  - everything else → Google SSO (you only)    │
                  └───────────────────────┬───────────────────────┘
                                          ▼
                            127.0.0.1:5050 (Rust axum)
                                          │
                ┌─────────────────────────┼─────────────────────────┐
                ▼                         ▼                         ▼
       SQLite (~/.tripit/db.sqlite)   Anthropic API          Embedded SPA
                                      (intake parsing)       (rust-embed)
```

Inbound email path is separate and asynchronous:

```
  trips@in.nathaninge.com  →  Cloudflare Email Worker
                                    │
                                    ▼  (HTTPS POST, HMAC-signed)
                            travel.nathaninge.com/api/intake/email
```

The Email Worker is a thin shim that reposts the parsed email JSON to the app.
CF Access has a Bypass policy for that one path *plus* a service-token check,
so the Worker can reach it but the public can't.

## 2. Stack

| Layer    | Choice                                                                       |
| -------- | ---------------------------------------------------------------------------- |
| Backend  | Rust 1.94+, `axum`, `sqlx` (sqlite), `tokio`, `utoipa`, `reqwest`            |
| LLM      | Anthropic Claude (Sonnet 4.6 default for parsing; Haiku 4.5 as cheap option) |
| Database | SQLite (WAL, foreign keys on, file at `~/.tripit/db.sqlite`)                 |
| Frontend | React 18, Vite, TypeScript, Tailwind, React Router, vite-pwa                 |
| Auth     | Cloudflare Access JWT verification + opaque share tokens                     |
| Email    | Cloudflare Email Routing → Email Worker → HMAC-signed webhook                |
| Tunnel   | `cloudflared` on the CloudDesktop                                            |

## 3. Data model

All ids are UUIDv7 strings (sortable, time-prefixed) unless noted.

```sql
CREATE TABLE trips (
  id              TEXT PRIMARY KEY,
  owner_email     TEXT NOT NULL,         -- the CF Access identity
  name            TEXT NOT NULL,
  destination     TEXT,                  -- free text summary
  start_date      DATE NOT NULL,
  end_date        DATE NOT NULL,
  notes_md        TEXT NOT NULL DEFAULT '',
  created_at      TEXT NOT NULL,         -- ISO8601 UTC
  updated_at      TEXT NOT NULL
);

CREATE TABLE travelers (
  id              TEXT PRIMARY KEY,
  trip_id         TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  sort_order      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE items (
  id              TEXT PRIMARY KEY,
  trip_id         TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
  type            TEXT NOT NULL CHECK (type IN ('flight','lodging','transport','activity')),
  title           TEXT NOT NULL,
  start_at        TEXT NOT NULL,         -- ISO8601 with offset
  end_at          TEXT,                  -- ISO8601 with offset; nullable
  location        TEXT,
  confirmation_no TEXT,
  notes           TEXT NOT NULL DEFAULT '',
  -- type-specific fields as a JSON blob (validated app-side per type)
  details_json    TEXT NOT NULL DEFAULT '{}',
  -- reference links: JSON array of {label, url} (see §5.6 of REQUIREMENTS)
  links_json      TEXT NOT NULL DEFAULT '[]',
  raw_source      TEXT,                  -- the original pasted / forwarded text
  source          TEXT NOT NULL CHECK (source IN ('paste','email','manual')),
  parse_status    TEXT NOT NULL DEFAULT 'parsed'
                    CHECK (parse_status IN ('parsed','review','manual')),
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

CREATE INDEX idx_items_trip_start ON items(trip_id, start_at);

CREATE TABLE item_travelers (
  item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  traveler_id TEXT NOT NULL REFERENCES travelers(id) ON DELETE CASCADE,
  PRIMARY KEY (item_id, traveler_id)
);

CREATE TABLE share_links (
  id              TEXT PRIMARY KEY,
  trip_id         TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
  label           TEXT,                  -- "Em's edit link"
  scope           TEXT NOT NULL CHECK (scope IN ('view','edit')),
  token_hash      TEXT NOT NULL UNIQUE,  -- sha256(token); raw token never stored
  created_at      TEXT NOT NULL,
  last_used_at    TEXT,
  revoked_at      TEXT
);
CREATE INDEX idx_share_token_hash ON share_links(token_hash) WHERE revoked_at IS NULL;

CREATE TABLE day_notes (
  trip_id     TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
  date        TEXT NOT NULL,             -- YYYY-MM-DD, trip-local
  notes_md    TEXT NOT NULL DEFAULT '',
  updated_at  TEXT NOT NULL,
  PRIMARY KEY (trip_id, date)
);

CREATE TABLE intake_inbox (
  id              TEXT PRIMARY KEY,      -- emails that arrived but couldn't be auto-assigned
  raw_text        TEXT NOT NULL,
  from_addr       TEXT,
  subject         TEXT,
  received_at     TEXT NOT NULL,
  status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending','assigned','discarded')),
  assigned_item_id TEXT REFERENCES items(id) ON DELETE SET NULL
);
```

Type-specific fields go in `details_json`. Examples:

```jsonc
// flight
{ "airline": "JL", "flight_no": "JL043", "dep_iata": "SFO", "dep_terminal": "I",
  "arr_iata": "HND", "arr_terminal": "3", "booking_ref": "ABC123", "seats": ["27A","27B"] }

// lodging
{ "address": "1-1 ...", "room_type": "Twin", "guests": 2, "host_contact": "..." }

// transport
{ "carrier": "JR", "vehicle_no": "Nozomi 7", "pickup": "Tokyo", "dropoff": "Kyoto" }

// activity
{ "venue": "TeamLab Planets", "party_size": 2 }
```

`links_json` is uniform across types:

```jsonc
[ { "label": "Manage booking",  "url": "https://www.delta.com/mytrips?confirmation=ABC123" },
  { "label": "Web check-in",    "url": "https://www.delta.com/checkin" },
  { "label": "Open in Maps",    "url": "https://maps.google.com/?q=..." } ]
```

A typed Rust enum (`ItemDetails`) deserializes from `details_json` based on
`type`; both write and read paths validate.

## 4. API surface

Convention: REST under `/api`. JSON. OpenAPI at `/api/openapi.json` (utoipa).

### 4.1 Owner-authenticated routes
Cloudflare Access JWT is in the `Cf-Access-Jwt-Assertion` header; middleware
verifies signature against the team's JWKS, checks `aud`, and pulls `email`.

| Method | Path                                  | Purpose                              |
| ------ | ------------------------------------- | ------------------------------------ |
| GET    | `/api/trips`                          | List trips                           |
| POST   | `/api/trips`                          | Create                               |
| GET    | `/api/trips/:id`                      | Fetch trip + items + travelers       |
| PATCH  | `/api/trips/:id`                      | Update metadata                      |
| DELETE | `/api/trips/:id`                      | Delete                               |
| POST   | `/api/trips/:id/items`                | Create item (manual)                 |
| POST   | `/api/trips/:id/items/parse`          | Parse pasted text → preview item     |
| PATCH  | `/api/items/:id`                      | Update                               |
| DELETE | `/api/items/:id`                      | Delete                               |
| POST   | `/api/trips/:id/travelers`            | Add traveler                         |
| DELETE | `/api/travelers/:id`                  | Remove                               |
| GET    | `/api/trips/:id/day-notes`            | Map of `date → notes_md`             |
| PUT    | `/api/trips/:id/day-notes/:date`      | Upsert day note (empty body clears)  |
| GET    | `/api/trips/:id/share`                | List share links                     |
| POST   | `/api/trips/:id/share`                | Mint a new share link (returns token *once*) |
| DELETE | `/api/share/:id`                      | Revoke                               |
| GET    | `/api/inbox`                          | List unassigned forwarded emails     |
| POST   | `/api/inbox/:id/assign`               | Assign to trip (re-parse + insert)   |
| DELETE | `/api/inbox/:id`                      | Discard                              |
| GET    | `/api/trips/:id/gaps`                 | Computed gaps (see §6)               |

### 4.2 Share-token routes
Token is in the URL path; the backend does `sha256(token)` → `share_links` lookup.

| Method | Path                                  | Scope required                |
| ------ | ------------------------------------- | ----------------------------- |
| GET    | `/api/share/:token/trip`              | view                          |
| POST   | `/api/share/:token/items`             | edit                          |
| PATCH  | `/api/share/:token/items/:item_id`    | edit                          |
| DELETE | `/api/share/:token/items/:item_id`    | edit                          |
| PATCH  | `/api/share/:token/trip`              | edit                          |
| GET    | `/api/share/:token/trip/gaps`         | view                          |
| GET    | `/api/share/:token/day-notes`         | view                          |
| PUT    | `/api/share/:token/day-notes/:date`   | edit                          |

Edit routes via share tokens cannot touch `share_links` or `owner_email`.

### 4.3 Service-token route (email worker only)
| Method | Path                       | Purpose                              |
| ------ | -------------------------- | ------------------------------------ |
| POST   | `/api/intake/email`        | Receive forwarded email from CF Email Worker |

Authenticated by HMAC of body with a shared secret in `X-Tripit-Signature`.
This path additionally requires CF Access service-token headers; CF Access
policy is "Allow service tokens" on `/api/intake/email`.

## 5. Parsing pipeline

### 5.1 Prompt
A single `extract_item.md` system prompt instructs Claude to emit JSON
matching the discriminated union below. The user message contains:
- For paste: the raw text
- For email: `Subject:\n\nFrom:\n\nText body:\n\nHTML body (stripped):`

Use `messages.create` with `response_format` enforcement via JSON schema (tool
use with one `record_item` tool whose `input_schema` is the union schema).

Output schema (one of):
```jsonc
{ "type": "flight",
  "title": "JL043 SFO → HND",
  "start_at": "2026-10-12T11:25:00-07:00",
  "end_at":   "2026-10-13T15:05:00+09:00",
  "location": "SFO Terminal I",
  "confirmation_no": "ABC123",
  "details": { ... },
  "links": [
    { "label": "Manage booking", "url": "https://..." },
    { "label": "Web check-in",   "url": "https://..." }
  ],
  "notes": "" }
```

The prompt explicitly instructs Claude to (a) extract URLs that appear in
the confirmation body and label them by purpose (manage booking, web
check-in, map/directions, property page, ticket PDF, etc), and (b) **not**
fabricate URLs — only include links that appear verbatim in the source.

If Claude returns `"type": "unknown"`, the item is saved with
`parse_status='review'` and surfaced in the inbox.

### 5.2 Caching
- Use prompt caching: the system prompt is cached. Saves ~80% on input tokens
  for every parse after the first in a 5-minute window.
- Model default: `claude-sonnet-4-6`. Override with env `TRIPIT_PARSE_MODEL`.

### 5.3 Failure handling
- Network/timeout: store the raw text with `parse_status='review'`, no
  structured fields. User can retry from the UI.
- Malformed JSON: same.
- Cost cap: a daily token-budget check in config; over-budget parses fail
  closed with a clear error.

## 6. Gap detection

Pure function over `(trip, items)`. Recomputed on demand; not stored.

```rust
struct Gap {
    kind: GapKind,           // UncoveredNight | NoInbound | NoOutbound
    date: NaiveDate,         // trip-local date this gap applies to
    message: String,
}
```

Algorithm:
1. Build `nights = [start_date .. end_date)` (each represents the *night of*).
2. For each night, check if any `lodging` item covers it
   (`check_in_date ≤ night < check_out_date`). If none: emit `UncoveredNight`.
3. Inbound check: collect arrivals on `start_date` from any flight/transport.
   If none: emit `NoInbound`.
4. Outbound check: mirror for `end_date` departures. If none: emit
   `NoOutbound`.

Trip-local timezone resolution: walk lodging items in chronological order;
each night inherits the timezone of the lodging covering it (we infer
timezone from the lodging `start_at` offset). For uncovered nights, fall
back to the previous covered night's tz, then to the trip's first-known tz.

## 7. Auth & sharing implementation

### 7.1 Cloudflare Access JWT verification
- Verify `Cf-Access-Jwt-Assertion` on every non-public request.
- JWKS URL: `https://<team>.cloudflareaccess.com/cdn-cgi/access/certs`,
  cached for 1h.
- Allowed `aud` = the CF Access app's AUD tag (env: `CF_ACCESS_AUD`).
- Map `email` claim to `owner_email`. Reject if not in the allowlist
  (single value in env `OWNER_EMAIL`).

### 7.2 Cloudflare Access policies on the hostname
Configure in the CF dashboard, in order:
1. **Allow** path `/api/intake/email` to service token "tripit-email-worker".
2. **Bypass** path `/share/*` and `/api/share/*` (the app itself does token
   auth).
3. **Allow** everything else to email `nate8983@gmail.com`.

### 7.3 Share token format
- Mint: 32 random bytes → URL-safe base64 → 43-char token.
- Store `sha256(token)` in DB; return the raw token *once* on creation
  (modal: "copy this link, it won't be shown again").
- Token check: O(1) lookup on `idx_share_token_hash`.

## 8. Email intake

### 8.1 Cloudflare Email Worker
- Domain `nathaninge.com` already has CF DNS; add MX records for
  `in.nathaninge.com` pointing at CF's email-routing servers.
- Email Routing rule: `trips@in.nathaninge.com` → Email Worker.
- Worker code (TypeScript) parses with `postal-mime`, then POSTs:

```ts
const body = {
  from: email.from,
  subject: email.subject,
  text: parsed.text,
  html: parsed.html,
  received_at: new Date().toISOString(),
};
const sig = await hmac(env.SHARED_SECRET, JSON.stringify(body));
await fetch("https://travel.nathaninge.com/api/intake/email", {
  method: "POST",
  headers: {
    "X-Tripit-Signature": sig,
    "CF-Access-Client-Id":     env.CF_ACCESS_CLIENT_ID,
    "CF-Access-Client-Secret": env.CF_ACCESS_CLIENT_SECRET,
    "content-type": "application/json",
  },
  body: JSON.stringify(body),
});
```

### 8.2 Server handling
1. Verify HMAC. If mismatch → 401.
2. Decide target trip: pick the trip where
   `start_date - 7d ≤ today ≤ end_date + 1d`. If 0 or >1, drop into
   `intake_inbox`.
3. If exactly one active trip, parse and insert as an item with
   `source='email'`. Mark `parse_status='review'` if Claude returned unknown.

## 9. Frontend

### 9.1 Routes
- `/` — trip list (owner view).
- `/trips/:id` — itinerary (timeline view, gap callouts, add-item form).
- `/trips/:id/manage` — travelers, share links, danger zone.
- `/inbox` — pending email-intake items.
- `/share/:token` — guest entrypoint; client reads token from URL and uses it
  for all API calls. Same itinerary component as owner view, with
  add/edit/delete gated on `scope`.

### 9.2 PWA
- `vite-pwa` with the `injectManifest` strategy. Precache the shell.
- Runtime cache (StaleWhileRevalidate) for `/api/trips/:id` and
  `/api/share/:token/trip` so a freshly-loaded itinerary is available offline.
- No background sync, no offline write queue.

### 9.3 UI shape
- Mobile-first, Tailwind, same component vocabulary as CTO.
- Itinerary view is a vertical timeline grouped by day. Lodging shows as a
  banner spanning the day-card stack it covers. Gap banners interleave.
- **Day header** on each day-card has an inline Markdown editor for day
  notes (click-to-edit; empty days show a faint "+ add notes" affordance
  only when in edit mode).
- **Item card** shows: title, time(s), location, traveler chips, then a
  row of link pills (`Manage booking`, `Open in Maps`, …) followed by the
  confirmation number as a tap-to-copy chip.
- Paste-confirmation flow: textarea → "Parse" → preview card (editable
  fields, including the links list) → "Save".

## 10. Project layout

```
backend/
  src/
    main.rs
    config.rs
    db.rs               # sqlx pool, migrations
    auth/
      cf_access.rs      # JWT verification middleware
      share_token.rs    # token middleware
      service_token.rs  # email-worker route guard + HMAC verify
    routes/
      trips.rs
      items.rs
      share.rs
      intake.rs
      gaps.rs
    domain/
      gap.rs            # gap detection (pure)
      tz.rs             # trip-local tz resolution
      item_details.rs   # typed details enum (de)serialization
    parse/
      claude.rs         # Anthropic client, prompt, schema
    static_assets.rs    # rust-embed of frontend/dist
  migrations/
    0001_init.sql
frontend/
  src/
    pages/
      TripList.tsx
      Trip.tsx
      TripManage.tsx
      Inbox.tsx
      SharedTrip.tsx
    components/
      ItemCard.tsx
      ItemLinks.tsx
      Timeline.tsx
      DayHeader.tsx
      DayNotesEditor.tsx
      GapBanner.tsx
      PasteParseForm.tsx
      ShareLinkManager.tsx
    api/                # fetch wrappers; owner vs share-token variants
    hooks/
    lib/
ops/
  cloudflared.config.yml.example
  email-worker/         # CF Email Worker source + wrangler.toml
  tripit.service        # systemd unit
  deploy.sh
Makefile
README.md
REQUIREMENTS.md
TECH_SPEC.md
```

## 11. Configuration (env vars)

| Var                       | Default                                          | Notes                                     |
| ------------------------- | ------------------------------------------------ | ----------------------------------------- |
| `DATABASE_URL`            | `sqlite://~/.tripit/db.sqlite?mode=rwc`          |                                           |
| `BIND_ADDR`               | `127.0.0.1:5050`                                 | Different from CTO's :5000                |
| `CF_ACCESS_TEAM_DOMAIN`   | e.g. `nathan.cloudflareaccess.com`               | For JWKS fetch                            |
| `CF_ACCESS_AUD`           | (required)                                       | App AUD tag                               |
| `OWNER_EMAIL`             | `nate8983@gmail.com`                             | Single allowed owner                      |
| `ANTHROPIC_API_KEY`       | (required)                                       |                                           |
| `TRIPIT_PARSE_MODEL`      | `claude-sonnet-4-6`                              | Override per-request also possible        |
| `TRIPIT_DAILY_TOKEN_CAP`  | `200000`                                         | Fail-closed safety cap                    |
| `TRIPIT_EMAIL_HMAC_KEY`   | (required if email enabled)                      | Shared with the CF Email Worker           |
| `RUST_LOG`                | `tripit=info,tower_http=info`                    |                                           |

## 12. Deploy

Same shape as CTO — single binary, systemd user unit, Cloudflare Tunnel.

1. `make build` on the dev mac (or in CI) — produces
   `backend/target/release/tripit`.
2. `REMOTE=user@cd-host ops/deploy.sh` rsyncs + builds release-mode on the
   CloudDesktop, installs to `~/.local/bin/tripit`, drops
   `~/.config/systemd/user/tripit.service`, restarts.
3. `cloudflared` config (already running for CTO) gains an extra ingress:

```yaml
ingress:
  - hostname: todo.nathaninge.com
    service: http://127.0.0.1:5000
  - hostname: travel.nathaninge.com
    service: http://127.0.0.1:5050
  - service: http_status:404
```

4. Cloudflare Access application on `travel.nathaninge.com` with the three
   policies in §7.2.

5. (Optional, v1) Email Worker deployed to a CF Worker; MX records added for
   `in.nathaninge.com`.

## 13. Milestones

**M1 — core CRUD (no parsing, no sharing).**
Schema, migrations, owner auth via CF Access, manual item entry, timeline
view, day notes, item link pills + tap-to-copy confirmation #. Working
end-to-end at `travel.nathaninge.com`.

**M2 — paste-text parsing.**
Anthropic integration, structured-output prompt (incl. link extraction),
preview-before-save UX.

**M3 — sharing.**
Share-link CRUD, token middleware, CF Access bypass policy for `/share/*`,
guest UI.

**M4 — gap detection.**
Uncovered-night + first/last-day inbound/outbound, inline banners.

**M5 — email intake.**
CF Email Worker, HMAC service-token path, inbox triage UI.

**M6 — polish.**
PWA install, offline cache, mobile timeline refinements, OpenAPI publishing.

## 14. Risks / decisions to revisit

- **Claude cost** scales with intake volume. Daily token cap is the first
  guard; if it starts hitting the cap, drop to Haiku 4.5 for parsing.
- **CF Email Worker** is the recommended path but adds a second deployable
  outside the binary. If aversion is high, drop email intake from v1 — paste
  alone covers 90% of cases.
- **Share-token leakage** via screenshots / Slack pastes is the user's
  responsibility; revocation is the only defense. Acceptable for a personal
  trip tool.
- **Single owner.** Anchoring on `OWNER_EMAIL` keeps the model simple. If a
  second owner ever needs in, that's a schema change (`trips.owner_email` →
  `trip_owners` join), not a redesign.
