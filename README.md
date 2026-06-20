# TripItinerary

Self-hosted trip itinerary webapp. Drops every booking confirmation into one
chronological per-trip view, flags obvious holes (uncovered nights, missing
inbound/outbound), and lets you share trips with travel companions via
unguessable links.

Sibling to [ContextualTaskOrganizer](../ContextualTaskOrganizer) — same single-
binary deploy shape, served at `travel.nathaninge.com` behind Cloudflare Access.

See [REQUIREMENTS.md](./REQUIREMENTS.md) and [TECH_SPEC.md](./TECH_SPEC.md) for
the full design. Currently shipping milestone **M1 — core CRUD**.

## Quickstart — local dev on Mac

Two terminals:

```bash
# 1) backend on :5050
cd backend && DEV_BYPASS_AUTH=1 cargo run

# 2) frontend dev server on :3001 (proxies /api → :5050)
cd frontend && npm install && npm run dev
```

`DEV_BYPASS_AUTH=1` skips the Cloudflare Access JWT check; required for local
work since CF Access only signs requests through the tunnel.

Open <http://localhost:3001>.

## Stack

| Layer    | Choice                                                          |
| -------- | --------------------------------------------------------------- |
| Backend  | Rust + axum + sqlx (sqlite) + tokio + utoipa                    |
| Database | SQLite (WAL, FK on) at `~/.tripit/db.sqlite`                    |
| Frontend | React 18, Vite, TypeScript, Tailwind, React Router, vite-pwa    |
| Auth     | Cloudflare Access JWT + opaque per-trip share tokens            |
| Parsing  | Anthropic Claude (Sonnet 4.6 default) — server-side, M2+        |

## Layout

```
backend/   Rust + axum + sqlx + utoipa; embeds frontend/dist via rust-embed
frontend/  React + Vite + Tailwind + PWA
ops/       systemd unit, cloudflared example, email-worker (M5+)
```
