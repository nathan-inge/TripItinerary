# TripItinerary

Self-hosted trip itinerary webapp. Paste or forward your booking
confirmations and get one chronological, day-by-day view of the trip that
your travel companions can read and edit with you.

Ships as a single Rust binary (SPA embedded) + one SQLite file.

## Features

**Itinerary**
- Day-by-day timeline of flights, lodging, transport, and activities.
  Multi-day stays and journeys get continuation banners on every day they
  span.
- Past days collapse automatically; today and upcoming days stay open.
- Every item location links straight to Google Maps.
- Attach links to any item (check-in page, tickets, menu).

**Getting confirmations in**
- Paste a confirmation email and an LLM extracts the structured fields.
  Low-confidence parses are flagged for review before saving.
- Or forward it to an intake address, with the trip name on the first line.
  Only senders on the trip's allow-list are accepted.
- Parse with Anthropic Claude or fully locally with Ollama. Before anything
  reaches a model, card numbers, phone numbers, emails, traveler names, and
  booking references are redacted, and the booking references are swapped
  back in locally afterwards.

**Planning together**
- Rich-text notes for each day, editable by anyone with edit access, for
  the plan, reservations to make, or ideas.
- Trip-wide todo list, with an open-count badge on the trip list.
- Tag each item with the travelers it applies to.
- Trip-level links to a shared photo album and a Google Maps list.

**Sharing**
- Share a trip by email (it shows up on their trip list) or by an
  unguessable link for guests who shouldn't need an account.
- Owner, edit, and view roles. Links can be revoked.
- Installable PWA for use on your phone while traveling.

See [REQUIREMENTS.md](./REQUIREMENTS.md) and [TECH_SPEC.md](./TECH_SPEC.md) for
the full design.

## Self-host with Docker

```bash
cp .env.example .env      # set OWNER_EMAIL; optionally a parse backend
docker compose up -d --build
open http://localhost:5050
```

That's a no-login local trial. For real use (behind a reverse proxy,
Tailscale, or Cloudflare Access) and fully-local parsing with Ollama, see
**[SELF_HOSTING.md](./SELF_HOSTING.md)**.

## Quickstart — local dev on Mac

```bash
# first time: build the frontend so rust-embed has something to bundle
make build-frontend

# terminal A — backend on :5050
cd backend && TRIPIT_AUTH_MODE=none OWNER_EMAIL=you@example.com cargo run

# terminal B — frontend dev server on :3001 (proxies /api → :5050, HMR)
cd frontend && npm run dev
```

`TRIPIT_AUTH_MODE=none` skips authentication and acts as the first
`OWNER_EMAIL` (needed for local dev: CF Access only signs requests that come
through the tunnel).

For paste-text parsing locally, also set `ANTHROPIC_API_KEY`.

Open <http://localhost:3001>.

## Deploy (bare metal)

A Linux server behind a Cloudflare Tunnel + Cloudflare Access. See
**[DEPLOY.md](./DEPLOY.md)** for the end-to-end walkthrough (one-time tunnel
+ CF Access setup, then `make deploy REMOTE=<ssh-host>` thereafter).

Configuration lives in `~/.config/tripit/env`; see [`.env.example`](./.env.example).

## Stack

| Layer    | Choice                                                          |
| -------- | --------------------------------------------------------------- |
| Backend  | Rust + axum + sqlx (sqlite) + tokio + utoipa                    |
| Database | SQLite (WAL, FK on) at `~/.tripit/db.sqlite`                    |
| Frontend | React 18, Vite, TypeScript, Tailwind, React Router, vite-pwa    |
| Auth     | Cloudflare Access JWT or trusted proxy header (+ per-trip email membership) and opaque share tokens |
| Parsing  | Anthropic Claude or local Ollama, with PII redaction first     |

## Layout

```
backend/   Rust + axum + sqlx + utoipa; embeds frontend/dist via rust-embed
frontend/  React + Vite + Tailwind + PWA
ops/       systemd unit, cloudflared example, deploy.sh, email-worker
```

## Make targets

| Target           | What it does                                                       |
| ---------------- | ------------------------------------------------------------------ |
| `dev-backend`    | `cargo run` on the backend (binds 127.0.0.1:5050)                  |
| `dev-frontend`   | Vite dev server on :3001 with `/api` proxy                         |
| `build`          | Build the SPA, then release-build the Rust binary                  |
| `build-frontend` | Just the SPA build (rust-embed needs `frontend/dist/` to exist)    |
| `test`           | `cargo test` + (frontend tests if any)                             |
| `deploy`         | Runs `ops/deploy.sh`; override `REMOTE=...`                        |
| `install-service`| Install + enable the systemd unit (run on the server)        |

## API

REST under `/api`. Health is unauthenticated; everything else needs either a
signed-in identity (per `TRIPIT_AUTH_MODE`: the trip's owner, or an address
it's been shared with) or a share token in the URL (guests). Each trip response carries `my_role` —
`owner`, `edit`, or `view` — which is what the UI gates on. The OpenAPI 3 spec
is published at `/api/openapi.json`.

## License

[MIT](./LICENSE)
