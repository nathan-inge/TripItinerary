# TripItinerary

Self-hosted trip itinerary webapp. Drops every booking confirmation into one
chronological per-trip view, flags obvious holes (uncovered nights, missing
inbound/outbound), and lets you share trips with travel companions via
unguessable links.

Sibling to [ContextualTaskOrganizer](../ContextualTaskOrganizer) — same single-
binary deploy shape, served at `travel.nathaninge.com` behind Cloudflare Access.

See [REQUIREMENTS.md](./REQUIREMENTS.md) and [TECH_SPEC.md](./TECH_SPEC.md) for
the full design. Currently shipping **M1 (CRUD)**, **M2 (paste parsing)**, and
**M3 (sharing)**. M4 (gap detection) and M5 (email intake) are next.

## Quickstart — local dev on Mac

```bash
# first time: build the frontend so rust-embed has something to bundle
make build-frontend

# terminal A — backend on :5050
cd backend && DEV_BYPASS_AUTH=1 cargo run

# terminal B — frontend dev server on :3001 (proxies /api → :5050, HMR)
cd frontend && npm run dev
```

`DEV_BYPASS_AUTH=1` skips Cloudflare Access JWT verification (required for
local dev — CF Access only signs requests that come through the tunnel).

For paste-text parsing locally, also set `ANTHROPIC_API_KEY`.

Open <http://localhost:3001>.

## Deploy

`travel.nathaninge.com` behind Cloudflare Access on a CloudDesktop. See
**[DEPLOY.md](./DEPLOY.md)** for the end-to-end walkthrough (one-time tunnel
+ CF Access setup, then `make deploy` thereafter — defaults to remote `marvin`).

Configuration lives in `~/.config/tripit/env`; see [`.env.example`](./.env.example).

## Stack

| Layer    | Choice                                                          |
| -------- | --------------------------------------------------------------- |
| Backend  | Rust + axum + sqlx (sqlite) + tokio + utoipa                    |
| Database | SQLite (WAL, FK on) at `~/.tripit/db.sqlite`                    |
| Frontend | React 18, Vite, TypeScript, Tailwind, React Router, vite-pwa    |
| Auth     | Cloudflare Access JWT + opaque per-trip share tokens            |
| Parsing  | Anthropic Claude (Sonnet 4.6 default) — server-side             |

## Layout

```
backend/   Rust + axum + sqlx + utoipa; embeds frontend/dist via rust-embed
frontend/  React + Vite + Tailwind + PWA
ops/       systemd unit, cloudflared example, deploy.sh, email-worker (M5)
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
| `install-service`| Install + enable the systemd unit (run on the CloudDesktop)        |

## API

REST under `/api`. Health is unauthenticated; everything else needs either a
Cloudflare Access JWT (owner) or a share token in the URL (guests). The
OpenAPI 3 spec is published at `/api/openapi.json`.
