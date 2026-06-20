# Deploy

End-to-end guide to standing up `travel.nathaninge.com` from a fresh
CloudDesktop. Mirrors the ContextualTaskOrganizer deploy shape so the same
host can run both apps.

## Prereqs

- A Linux CloudDesktop reachable via SSH (`marvin` — Nathan's CloudDesktop host).
- `cloudflared` installed on the CloudDesktop with a Tunnel that fronts
  `*.nathaninge.com`. If you already deployed CTO this is done.
- A Cloudflare Access "team" set up at `<team>.cloudflareaccess.com`.
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

On the CloudDesktop, edit `/etc/cloudflared/config.yml` and add the new
hostname. See [`ops/cloudflared.config.yml.example`](./ops/cloudflared.config.yml.example)
for the full shape. The relevant block:

```yaml
ingress:
  - hostname: travel.nathaninge.com
    service: http://127.0.0.1:5050
  # keep your existing entries before the catch-all
  - service: http_status:404
```

Route DNS and reload:

```bash
sudo cloudflared tunnel route dns <TUNNEL_NAME> travel.nathaninge.com
sudo systemctl restart cloudflared
```

## 3. Cloudflare Access: protect the subdomain

In the Cloudflare dashboard → Zero Trust → Access → Applications, create a
**Self-hosted application** for `travel.nathaninge.com` with the policies
below, in this exact order. Order matters — first match wins.

| # | Action  | Name                  | Selector                                  | Notes                                              |
|---|---------|-----------------------|-------------------------------------------|----------------------------------------------------|
| 1 | Allow   | Email worker          | Path `/api/intake/email` + Service token | M5; safe to add now with no service token yet.    |
| 2 | Bypass  | Share links           | Path `/share/*` OR `/api/share/*`         | Token auth happens in the app.                    |
| 3 | Allow   | Owner                 | Email is `nate8983@gmail.com`             | Catch-all for everything else.                    |

After creating the app, copy the **AUD tag** from the application's
configuration (Application → "Application Audience (AUD) Tag"). You'll
need it as `CF_ACCESS_AUD` below.

## 4. Provision secrets on the CloudDesktop

```bash
mkdir -p ~/.config/tripit
cat > ~/.config/tripit/env <<'EOF'
ANTHROPIC_API_KEY=sk-ant-...
CF_ACCESS_TEAM_DOMAIN=nathan.cloudflareaccess.com
CF_ACCESS_AUD=<AUD-from-step-3>
OWNER_EMAIL=nate8983@gmail.com
# Optional overrides:
# TRIPIT_PARSE_MODEL=claude-haiku-4-5-20251001   # cheaper fallback
# TRIPIT_DAILY_TOKEN_CAP=200000
EOF
chmod 600 ~/.config/tripit/env
```

The systemd unit (`ops/tripit.service`) reads this file at startup via
`EnvironmentFile=-%h/.config/tripit/env`. The leading dash means systemd
won't refuse to start if it's missing.

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
- **403 from Cloudflare Access on shared link** — the Bypass rule on
  `/share/*` is missing or after the catch-all. Check policy ordering.
- **JWT errors in logs** — `CF_ACCESS_AUD` is wrong. Re-copy from the CF
  Access application's config page.
- **Parsing returns 400** — `ANTHROPIC_API_KEY` is unset in
  `~/.config/tripit/env`. Set it and `systemctl --user restart tripit`.
