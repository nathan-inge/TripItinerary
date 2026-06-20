# Deploy

End-to-end guide to standing up `travel.nathaninge.com` from a fresh
CloudDesktop. Mirrors the ContextualTaskOrganizer deploy shape so the same
host can run both apps.

## Prereqs

- A Linux CloudDesktop reachable via SSH (`marvin` — Nathan's CloudDesktop host).
- `cloudflared` installed on the CloudDesktop with a Tunnel that fronts
  `*.nathaninge.com`. If you already deployed CTO this is done.
- A Cloudflare Access "team" set up at `<team>.cloudflareaccess.com`.
  Find your team name at **Zero Trust → Settings → Custom Pages (or
  General) → Team domain**.
- An `ANTHROPIC_API_KEY` (for paste-text parsing, M2+).

## 1. CloudDesktop: install Rust + Node

```bash
ssh marvin
# Rust 1.94+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
# Node 20+
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs build-essential pkg-config
```

## 2. Cloudflare Tunnel: add the `travel.nathaninge.com` ingress

`/etc/cloudflared/config.yml` is root-owned, so edit with sudo:

```bash
sudoedit /etc/cloudflared/config.yml      # or: sudo nano /etc/cloudflared/config.yml
```

Add the new hostname to the `ingress` list, **before** the `http_status:404`
catch-all. See [`ops/cloudflared.config.yml.example`](./ops/cloudflared.config.yml.example)
for the full shape. The relevant block:

```yaml
ingress:
  - hostname: travel.nathaninge.com
    service: http://127.0.0.1:5050
  # keep your existing entries before the catch-all
  - service: http_status:404
```

Validate before restarting — catches indent errors while the tunnel is still
up:

```bash
sudo cloudflared --config /etc/cloudflared/config.yml tunnel ingress validate
sudo cloudflared tunnel route dns <TUNNEL_NAME> travel.nathaninge.com
sudo systemctl restart cloudflared
sudo systemctl status cloudflared --no-pager
```

## 3. Cloudflare Access: protect the subdomain

The trick: this hostname needs **two** Access Applications, scoped by path,
because the share-link flow has to skip SSO entirely while the rest of the
API needs your Google identity. Everything outside `/api/*` (the SPA shell,
manifest, JS bundles) is implicitly unprotected — that's intentional, since
the bundle contains no secrets and all data sits behind the API.

In Cloudflare dashboard → **Zero Trust → Access → Applications**, create
both apps under the same `travel.nathaninge.com` hostname:

### App 1 — Share-link bypass

| Field           | Value                                  |
|-----------------|----------------------------------------|
| Type            | Self-hosted                            |
| Name            | `tripit share bypass`                  |
| Domain          | `travel.nathaninge.com`                |
| **Path**        | `/api/share/*`                         |
| Session         | (anything, irrelevant for Bypass)      |

One policy:

| Field    | Value     |
|----------|-----------|
| Name     | `Bypass`  |
| Action   | **Bypass** |
| Include  | **Everyone** |

### App 2 — Owner (SSO)

| Field           | Value                                  |
|-----------------|----------------------------------------|
| Type            | Self-hosted                            |
| Name            | `tripit owner`                         |
| Domain          | `travel.nathaninge.com`                |
| **Path**        | `/api/*`                               |
| Session         | 24h (or whatever you prefer)           |

One policy:

| Field    | Value                                       |
|----------|---------------------------------------------|
| Name     | `Owner`                                     |
| Action   | **Allow**                                   |
| Include  | **Emails** → `nate8983@gmail.com`           |

Cloudflare matches the most-specific path first, so `/api/share/<token>/...`
hits App 1 (Bypass) and everything else under `/api/*` hits App 2 (SSO
required). Paths outside `/api/*` are not gated by any Application and pass
through to the app, which serves the SPA shell.

After saving **App 2**, copy its **AUD tag** from the Overview page (long
hex string). That's the one — and only one — that goes in `CF_ACCESS_AUD`
below. App 1 issues no JWT (Bypass passes the request through unmodified),
so its AUD is unused.

> **M5 deferred.** When email intake (M5) ships, you'll add a third App at
> path `/api/intake/email` with Action = Service Auth and a Service Token
> created in Zero Trust → Access → Service Auth. It needs its own service
> token before it can be saved.

## 4. Provision secrets on the CloudDesktop

```bash
mkdir -p ~/.config/tripit
cat > ~/.config/tripit/env <<'EOF'
ANTHROPIC_API_KEY=sk-ant-...
CF_ACCESS_TEAM_DOMAIN=<your-team>.cloudflareaccess.com
CF_ACCESS_AUD=<App 2 AUD from step 3>
OWNER_EMAIL=nate8983@gmail.com
# Optional overrides:
# TRIPIT_PARSE_MODEL=claude-haiku-4-5-20251001   # cheaper fallback
# TRIPIT_DAILY_TOKEN_CAP=200000
EOF
chmod 600 ~/.config/tripit/env
```

The systemd unit (`ops/tripit.service`) reads this file at startup via
`EnvironmentFile=-%h/.config/tripit/env`. The leading dash means systemd
won't refuse to start if the file is missing.

## 5. Deploy

From your Mac:

```bash
make deploy
```

(`marvin` is the default remote; override with `REMOTE=other-host make deploy`.)

That rsyncs the source, runs `make build` remotely, installs the binary at
`~/.local/bin/tripit` and the unit at `~/.config/systemd/user/tripit.service`,
then `daemon-reload + restart`. It also runs `loginctl enable-linger` so
the user-session persists when you log out.

Verify on the CloudDesktop:

```bash
systemctl --user status tripit
journalctl --user -u tripit -f
curl http://127.0.0.1:5050/api/health
```

## 6. First-time use

1. Open `https://travel.nathaninge.com` on a logged-in browser. Cloudflare
   Access redirects you to Google, you authenticate, the JWT lands, and
   the app loads.
2. Create a trip.
3. Mint a share link from the Manage page, copy the URL, send to a guest.
   The guest URL is `https://travel.nathaninge.com/share/<token>` — no
   account needed on their end.

## Subsequent deploys

```bash
make deploy
```

Same idempotent flow. SQLite at `~/.tripit/db.sqlite` is untouched.

## Backup

```bash
# on the CloudDesktop
0 3 * * * cp ~/.tripit/db.sqlite ~/.tripit/backups/db-$(date +\%F).sqlite
```

## Rollback

The binary is just one file. Keep the previous one around:

```bash
# before each deploy, on the CloudDesktop:
cp ~/.local/bin/tripit ~/.local/bin/tripit.prev
```

To roll back: `cp ~/.local/bin/tripit.prev ~/.local/bin/tripit && systemctl --user restart tripit`.

If migrations have moved forward and you need to roll the schema back, restore the SQLite file from a backup before downgrading the binary.

## Troubleshooting

- **`tripit.service` won't start** — `journalctl --user -u tripit -n 50`.
  Most common: missing `~/.tripit/` (the unit creates this on first run
  but only if it can write to `$HOME`).
- **Share link → CORS error fetching `/manifest.webmanifest` (or `/assets/*`)**
  — App 2 (Owner) is scoped to the whole hostname instead of `/api/*`.
  Re-scope App 2 to path `/api/*` so static assets pass through with no
  Access enforcement. Hard-reload (Cmd-Shift-R) to drop the cached redirect
  and the stale service worker.
- **`401 unauthorized` on every share-link API call** — App 1's path is
  wrong. It must be `/api/share/*`, not `/share/*`. The frontend route
  `/share/<token>` is just the SPA shell entry point; the actual data
  fetches go to `/api/share/<token>/trip`.
- **JWT errors in logs (`kid not in JWKS`, `invalid audience`)** —
  `CF_ACCESS_AUD` or `CF_ACCESS_TEAM_DOMAIN` is wrong. Re-copy AUD from
  App 2's Overview page; team domain from Zero Trust → Settings.
- **Parsing returns 400** — `ANTHROPIC_API_KEY` is unset in
  `~/.config/tripit/env`. Set it and `systemctl --user restart tripit`.
