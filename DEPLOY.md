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

This hostname needs **two** Access Applications. The owner App must gate
the **entire hostname** (not just `/api/*`) so the SSO challenge happens at
page-load time — browsers can follow SSO redirects on a navigation, but
they can't follow them on a cross-origin `fetch()` (CORS blocks the
preflight). Gating only `/api/*` would mean the SPA boots without a
session, then its first `fetch('/api/trips')` triggers a 302 → CORS error.

The Bypass App therefore has to enumerate every path a guest needs to
reach without SSO — the share routes and the SPA's static assets. The
shell contains no secrets; bypassing it is safe.

In Cloudflare dashboard → **Zero Trust → Access → Applications**, create
both apps under the same `travel.nathaninge.com` hostname:

### App 1 — Public bypass

| Field   | Value                                                                                      |
|---------|--------------------------------------------------------------------------------------------|
| Type    | Self-hosted                                                                                |
| Name    | `tripit public bypass`                                                                     |
| Domain  | `travel.nathaninge.com`                                                                    |
| **Paths** | Add **each** as a separate entry in the Paths list:                                      |
|         | `/share/*` — guest SPA route                                                               |
|         | `/api/share/*` — guest API                                                                 |
|         | `/assets/*` — Vite-fingerprinted JS/CSS bundles                                            |
|         | `/manifest.webmanifest` — PWA manifest                                                     |
|         | `/sw.js` — service worker                                                                  |
|         | `/workbox-*` — service-worker deps                                                         |
|         | `/icon.svg` — PWA icon                                                                     |

One policy:

| Field    | Value        |
|----------|--------------|
| Name     | `Bypass`     |
| Action   | **Bypass**   |
| Include  | **Everyone** |

### App 2 — Owner (SSO, whole hostname)

| Field      | Value                                                  |
|------------|--------------------------------------------------------|
| Type       | Self-hosted                                            |
| Name       | `tripit owner`                                         |
| Domain     | `travel.nathaninge.com`                                |
| **Path**   | *(leave blank — covers the whole hostname)*            |
| Session    | 24h (or whatever you prefer)                           |

One policy:

| Field    | Value                                       |
|----------|---------------------------------------------|
| Name     | `Owner`                                     |
| Action   | **Allow**                                   |
| Include  | **Emails** → `nate8983@gmail.com`           |

Cloudflare matches the most-specific path first, so the App 1 paths
(share routes + static assets) hit Bypass; everything else on the
hostname (`/`, `/trips/<id>`, `/api/trips`, `/api/health`, …) hits Owner
SSO.

After saving **App 2**, copy its **AUD tag** from the Overview page (long
hex string). That's the one — and only one — that goes in `CF_ACCESS_AUD`
below. App 1 issues no JWT (Bypass passes the request through unmodified),
so its AUD is unused.

> **For email intake (optional, see §7).** When you stand up the Email
> Worker, you'll add a third App scoped to path `/api/intake/email` with
> Action = Service Auth and a Service Token. Skip until you're ready to
> wire up forwarding.

## 4. Provision secrets on the CloudDesktop

```bash
mkdir -p ~/.config/tripit
cat > ~/.config/tripit/env <<'EOF'
CF_ACCESS_TEAM_DOMAIN=<your-team>.cloudflareaccess.com
CF_ACCESS_AUD=<App 2 AUD from step 3>
OWNER_EMAIL=nate8983@gmail.com

# Pick a parse backend (or omit to skip auto-extraction entirely — paste/
# forward will store the raw text and you fill fields by hand).
#
# Option A (cloud, easier — confirmation text PII-redacted, then sent to
# Anthropic; data not used for training, deleted at ≤30 days):
ANTHROPIC_API_KEY=sk-ant-...
# TRIPIT_PARSE_MODEL=claude-sonnet-4-6

# Option B (local, recommended for privacy — text never leaves the host):
# TRIPIT_PARSE_BACKEND=ollama
# TRIPIT_OLLAMA_URL=http://localhost:11434
# TRIPIT_OLLAMA_MODEL=qwen2.5:7b-instruct
# See §8 below for Ollama install.

# TRIPIT_DAILY_TOKEN_CAP=200000
# TRIPIT_EMAIL_HMAC_KEY=<hex from openssl rand -hex 32>  # required for §7
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

## 7. Email intake (optional)

Lets allow-listed senders forward booking confirmations to a magic
address; the backend matches them to a trip by the trip-name line at the
top of the body and parses the rest with Claude. End-to-end setup:

### 7.1 DNS — MX records for the intake subdomain

The apex `nathaninge.com` keeps its existing mail. Put intake on a
subdomain (`in.nathaninge.com`) so they don't collide. In Cloudflare
DNS, add MX records for the `in` subdomain pointing at Cloudflare's
email-routing servers:

```
in.nathaninge.com   MX 10  route1.mx.cloudflare.net
in.nathaninge.com   MX 20  route2.mx.cloudflare.net
in.nathaninge.com   MX 50  route3.mx.cloudflare.net
```

### 7.2 Enable Email Routing on the subdomain

Dashboard → **Email → Email Routing** → enable for `in.nathaninge.com`.
(You'll briefly see a "verifying MX records" step. Wait until it goes
green.)

### 7.3 Mint shared secrets

```bash
# on your Mac
openssl rand -hex 32              # save as TRIPIT_EMAIL_HMAC_KEY everywhere
```

In Zero Trust → **Access → Service Auth → Service Tokens**, create a
token named `tripit-email-worker`. Save the **Client ID** and **Client
Secret** — you'll need both.

Add the HMAC key to the backend env on marvin and restart:

```bash
ssh marvin
echo 'TRIPIT_EMAIL_HMAC_KEY=<your-32-byte-hex>' >> ~/.config/tripit/env
systemctl --user restart tripit
```

### 7.4 CF Access App for `/api/intake/email`

In Zero Trust → Access → Applications, add a third app on
`travel.nathaninge.com`:

| Field   | Value                                                       |
|---------|-------------------------------------------------------------|
| Type    | Self-hosted                                                 |
| Name    | `tripit email worker`                                       |
| Domain  | `travel.nathaninge.com`                                     |
| **Path** | `/api/intake/email`                                        |

Policy:

| Field   | Value                                                       |
|---------|-------------------------------------------------------------|
| Name    | `Email worker`                                              |
| Action  | **Service Auth**                                            |
| Include | **Service Token** → `tripit-email-worker`                   |

Path-specificity puts this above the hostname-scoped Owner app, so the
Worker's POSTs are allowed; everything else still needs owner SSO.

### 7.5 Deploy the Worker

```bash
cd ops/email-worker
npm install
npx wrangler login                                   # if not already
npx wrangler secret put TRIPIT_EMAIL_HMAC_KEY        # same hex as backend
npx wrangler secret put CF_ACCESS_CLIENT_ID          # from §7.3
npx wrangler secret put CF_ACCESS_CLIENT_SECRET      # from §7.3
npx wrangler deploy
```

### 7.6 Route the address to the Worker

Email Routing → **Routing rules** → Create a rule:
- Match: `trips@in.nathaninge.com`
- Action: **Send to a Worker** → `tripit-email-worker`

### 7.7 Add allow-listed senders to a trip

In the app, open a trip's **Manage** page and use **Forward by email**
to add at least one sender (yourself, to start). They can then forward
a confirmation to `trips@in.nathaninge.com` with the trip name on the
first line of the body:

```
Japan, autumn 2026

---------- Forwarded message ----------
From: Delta Air Lines <noreply@delta.com>
...
```

Watch `journalctl --user -u tripit -f` while you test. The backend
silently drops emails whose trip-name doesn't match or whose sender
isn't on the allow-list — failures are logged, not bounced.

## 8. Privacy: local parsing with Ollama (optional)

Forwarded confirmations contain traveler names, PNRs, and sometimes
last-4 of card. The Anthropic API doesn't train on customer content
and retains for ≤30 days, but if you'd rather keep that text on the
host entirely, run the parser against a local Ollama instance.

The PII redactor runs on both backends — payment cards (Luhn-checked),
SSNs, phone numbers, and naked emails outside URLs are scrubbed before
the LLM sees them. With Ollama selected as the backend, nothing leaves
the host at all.

### 8.1 Install Ollama on marvin

```bash
ssh marvin
curl -fsSL https://ollama.com/install.sh | sh
# pulls ~4.7GB for qwen2.5; runs once at first inference
ollama pull qwen2.5:7b-instruct
# verify it answers
ollama run qwen2.5:7b-instruct "Reply with only the word ok."
```

Ollama installs as a systemd service listening on `127.0.0.1:11434`.

### 8.2 Switch tripit to the local backend

```bash
cat >> ~/.config/tripit/env <<'EOF'
TRIPIT_PARSE_BACKEND=ollama
TRIPIT_OLLAMA_URL=http://localhost:11434
TRIPIT_OLLAMA_MODEL=qwen2.5:7b-instruct
EOF
systemctl --user restart tripit
journalctl --user -u tripit -n 10 | grep 'parse backend'
# expect: "parse backend = ollama at http://localhost:11434 ..."
```

### 8.3 Model picks

| Model                       | Size | Notes                                                |
|-----------------------------|------|------------------------------------------------------|
| `qwen2.5:7b-instruct`       | 4.7G | Recommended. Strong at structured-output extraction. |
| `llama3.1:8b-instruct`      | 4.7G | Comparable; pick if Qwen JSON adherence is shaky.    |
| `qwen2.5:14b-instruct`      | 9.0G | Better recall on tricky multi-leg confirmations.     |
| `gemma2:9b-instruct`        | 5.4G | Decent alternative.                                  |

On CPU expect 5–20 s per parse for 7B models; sub-second on a GPU.
Email intake is async so latency isn't a UX issue.

### 8.4 Going back to Anthropic

Just remove `TRIPIT_PARSE_BACKEND=ollama` from the env (or set it to
`anthropic`) and `systemctl --user restart tripit`. The Anthropic
client still applies the same PII redactor before the request leaves.

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
- **Owner view: CORS error redirecting `/api/trips` → `<team>.cloudflareaccess.com/cdn-cgi/access/login/...`**
  — Owner App 2 is scoped to `/api/*` only, so the SPA shell loads
  without a CF Access session, and the first XHR triggers an SSO redirect
  that the browser can't follow cross-origin. Fix: re-scope App 2 to
  the **whole hostname** (no path), and add the SPA's static assets
  (`/assets/*`, `/manifest.webmanifest`, `/sw.js`, `/workbox-*`, `/icon.svg`)
  to App 1's Bypass paths so guests still load them without SSO. Hard-reload
  (Cmd-Shift-R) to drop the cached redirect and the stale service worker.
- **Share link → CORS error fetching `/manifest.webmanifest` (or `/assets/*`)**
  — Same root cause as above: a static-asset path isn't in App 1's
  Bypass list. Add it.
- **`401 unauthorized` on every share-link API call** — App 1 is missing
  `/api/share/*` from its Paths. The frontend route `/share/<token>` is
  just the SPA shell entry point; the data fetches go to `/api/share/<token>/trip`,
  which is a separate path entry.
- **JWT errors in logs (`kid not in JWKS`, `invalid audience`)** —
  `CF_ACCESS_AUD` or `CF_ACCESS_TEAM_DOMAIN` is wrong. Re-copy AUD from
  App 2's Overview page; team domain from Zero Trust → Settings.
- **Parsing returns 400** — `ANTHROPIC_API_KEY` is unset in
  `~/.config/tripit/env`. Set it and `systemctl --user restart tripit`.
- **Forwarded email doesn't appear on the trip** — most likely one of:
  (a) the body doesn't start with the exact trip name (case-insensitive,
  whitespace-normalized match — punctuation matters); (b) the sender's
  address isn't on the trip's allow-list (Manage → Forward by email);
  (c) the HMAC keys don't match between the Worker and the backend.
  Tail `journalctl --user -u tripit -f` while you send a test — the
  reason is logged at `INFO` level.
