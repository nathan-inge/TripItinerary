# TripItinerary — Requirements

A self-hosted webapp that turns a pile of travel confirmations (flights, hotels,
Airbnbs, museum tickets, restaurant reservations, train bookings) into a single,
linear, day-by-day itinerary per trip — and flags the obvious holes.

Vended at `travel.nathaninge.com`, behind Cloudflare Access for the owner, with
per-trip share tokens for guests.

Sibling project to ContextualTaskOrganizer; same deploy shape (single Rust
binary + embedded SPA + SQLite, Cloudflare Tunnel to a CloudDesktop).

## 1. Goals

1. **One place** to drop every booking confirmation for a trip and see the whole
   thing laid out chronologically.
2. **Minimum-effort intake**: paste the confirmation email body, or forward the
   email to a magic address — the app extracts the structured fields.
3. **Spot the holes** before the trip: nights without lodging, no inbound on
   day 1, no outbound on the last day.
4. **Share** a trip with travel companions (view-only or full-edit) without
   making them create accounts.

## 2. Non-goals (v1)

- Booking anything. Read-only consumer of existing confirmations.
- Price/cost tracking, budget rollups, expense splitting.
- Reminders, push notifications, SMS/email nudges. (Itinerary is pull-only.)
- Map view / route rendering.
- Native mobile app. PWA is the mobile path.
- Calendar export (`.ics`). Maybe v1.5.
- Document storage (boarding-pass PDFs, etc) beyond keeping the raw source text.
- General-purpose collaboration: comments, real-time presence, change history.

## 3. Users & roles

| Role          | Auth                         | Can do                                                                       |
| ------------- | ---------------------------- | ---------------------------------------------------------------------------- |
| **Owner** (you) | Cloudflare Access (Google SSO) | Everything: create/delete trips, add items, mint/revoke share links.       |
| **Editor**    | Share link with `edit` scope | Add/modify/delete items, edit trip metadata. Cannot manage other share links. |
| **Viewer**    | Share link with `view` scope | Read-only.                                                                   |

Share links are unguessable bearer tokens in the URL. They are revocable. No
guest signup, ever.

> **Editor scope decision**: "full edit" was chosen — editors can change trip
> dates and item details. They cannot mint or revoke share links; that stays
> with the owner.

## 4. Trip model (overview)

A **Trip** has:

- Name (e.g. "Japan, autumn 2026")
- Start date, end date (inclusive)
- Optional free-text destination summary
- **Travelers**: an explicit list of names (e.g. "Nate", "Em", "the kid").
  Items can be tagged with which travelers they apply to.
- Items (see §5)
- **Day notes**: per-day free-text Markdown (see §5.5)
- Share links (see §7)
- Notes (free-text Markdown, per trip, top-level)

A trip is a multi-segment thing — Tokyo → Kyoto → Osaka is *one* trip, not
three. The day grouping in the UI is driven by the timezone of whatever lodging
covers that night.

## 5. Item types

All items share: `id`, `trip_id`, `type`, `title`, `start_at` (timestamptz),
`end_at` (nullable timestamptz), `location` (free text), `confirmation_number`,
`notes`, `traveler_ids[]`, `links[]` (see §5.6), `raw_source` (the original
pasted/forwarded text), `created_at`, `updated_at`.

Four typed flavors in v1, each with extra fields:

### 5.1 Flight
- Airline, flight number
- Departure airport (IATA), departure terminal, departure local time
- Arrival airport (IATA), arrival terminal, arrival local time
- Seat assignment(s) (per traveler)
- Booking reference / record locator

### 5.2 Lodging (hotel / Airbnb / etc)
- Property name, address
- Check-in datetime (with timezone), check-out datetime (with timezone)
- Number of guests, room/unit type
- Host/contact info

### 5.3 Ground transport (train / bus / car rental / transfer)
- Carrier, vehicle/train number
- Pickup location + time, dropoff location + time
- For car rentals: vendor, pickup/dropoff, vehicle class

### 5.4 Activity / ticket / reservation
- Venue, address
- Start time, end time (nullable for "valid anytime today" tickets)
- Party size, ticket-holder names
- Notes (dress code, what to bring, etc)

### 5.5 Day notes (not an item, per-day free text)
Per trip-day, an optional Markdown blob for things that aren't bookings —
"wander Daikanyama in the morning", "try that ramen place near Shibuya",
"early night, big day tomorrow". Surfaced inline at the top of each day card
in the timeline. Editable by anyone with edit access to the trip. Empty days
show nothing (no placeholder).

### 5.6 Reference links / quick lookups
Every item carries `links[]` — a list of `{label, url}` pairs. Examples:

- Flight: "Manage booking" → `delta.com/mytrips?confirmation=ABC123`,
  "Web check-in" → `…`
- Lodging: "Manage reservation" → Airbnb / Booking.com URL,
  "Property website" → `…`, "Open in Maps" → `https://maps.…`
- Transport: "Manage booking" → carrier URL, "Print ticket" → PDF link
- Activity: "Ticket page", "Venue website", "Open in Maps"

Sources, in priority order:

1. **Parser-extracted**: the LLM scans the raw confirmation, picks out
   meaningful URLs, and labels them.
2. **Manual**: any link can be added/edited/removed on the item.

The item card UI surfaces them as a row of pill buttons under the title.
`confirmation_number` is always shown with a tap-to-copy affordance.

## 6. Intake

Two paths, **both** populate the same Item schema. Both rely on a server-side
LLM call (Anthropic API) to extract structured fields from unstructured email.

### 6.1 Paste raw text
- Big textarea on the trip page: "Paste a confirmation".
- App calls Claude with a structured-output prompt and the raw text.
- Result is shown for confirmation before save (user can edit any field).
- `raw_source` keeps the original paste, so re-parsing is always possible.

### 6.2 Email forwarding
- Each owner has a single inbound address, e.g.
  `trips@in.nathaninge.com`.
- Forwarding it adds an item to the **most recent active trip** by default,
  with a fallback "inbox" if no trip is plausibly current. (Editable.)
- Same Claude extraction; the email body (plain + HTML) is the input.
- A confirmation page lists newly-arrived items with a "Confirm / fix /
  reassign trip" review queue.

Either path always stores the raw source. Mis-parsed items can be re-parsed or
hand-corrected.

### 6.3 Manual entry
- Available as a fallback — typed form per item type. Sometimes you don't have
  an email (a friend booked the Airbnb, etc).

## 7. Sharing

### 7.1 Share-link types
- **View link**: read-only access to a trip.
- **Edit link**: full add/modify/delete on items and trip metadata, but no
  ability to mint or revoke share links.

### 7.2 Properties
- Each link is a long random token in the URL (`/share/<token>`).
- Owner can list, name (e.g. "Em's edit link"), and revoke links at any time.
- Links never expire automatically (revocation is explicit). Optional
  expiration is a v1.5 nice-to-have.
- Anyone with the URL has the access; no recipient identity is checked. We're
  optimizing for "send the link in iMessage, done".

### 7.3 Trip privacy
- Trips are private by default. Owner sees them via Cloudflare Access auth.
- The existence of a token-bearing URL does *not* make the trip discoverable;
  there's no trip listing for guests.

## 8. Linear itinerary view (the main thing)

- Items grouped by **day** in the trip-local timezone (driven by current
  lodging segment).
- Within a day, items sort by `start_at`.
- Flights show dep / arr times in each leg's local tz.
- Lodging spans show as a banner across the days they cover.
- Missing-item callouts appear inline at the relevant day(s) (see §9).
- Mobile-first layout. PWA so it works offline once cached.

## 9. Gap / missing-item detection

The app passively highlights two kinds of gap; it does not block saves or
nag.

### 9.1 Uncovered nights
For each night between trip start and trip end (exclusive of departure day if
there's an outbound flight that leaves before any reasonable bedtime), check
that at least one lodging item covers it (`check_in ≤ that night < check_out`).
Each uncovered night gets a banner:

> 🛏️ **No lodging booked for the night of Tue Oct 13.**

### 9.2 First/last day transport
- Trip start day with no flight/train/bus/car arriving on or before that day's
  end → "🛬 No inbound transport for day 1."
- Trip end day with no flight/train/bus/car departing on or after that day's
  start → "🛫 No outbound transport for day N."

### 9.3 Out of scope for v1
- Geographic gap between lodging locations.
- Tight-connection warnings.
- "Are you sure you'll be hungry?" reservation suggestions.

(Both deferred — geographic gaps especially require resolving free-text
locations, which is its own can of worms.)

## 10. Out-of-scope for v1 (consolidated)

- Reminders / notifications (none).
- Booking, pricing, expenses.
- Geographic gap / tight-connection detection.
- Map view.
- `.ics` export, Google Calendar sync.
- Document attachment storage beyond raw text.
- Multi-owner trips (one Cloudflare Access identity is the only owner; sharing
  is read-or-edit-via-token).

## 11. Resolved decisions

1. **Email intake**: Cloudflare Email Routing → Email Worker. Intake address
   is `trips@in.nathaninge.com` — a dedicated subdomain so the apex
   `nathaninge.com` MX records (real mail) stay untouched. The Worker
   HMAC-signs and POSTs to `/api/intake/email`.
2. **Default trip on email arrival**: pick the trip where
   `start_date - 7d ≤ today ≤ end_date + 1d`. Ambiguous (0 or >1 matches) →
   `intake_inbox` for manual assignment.
3. **PWA offline scope**: precache the shell; runtime
   StaleWhileRevalidate cache for the most-recently-viewed itinerary per
   trip. No offline write queue, no background sync.
4. **Trip-local timezone**: derived from the lodging covering each night;
   uncovered nights inherit the previous covered night's tz, then fall back
   to the trip's first known tz.
