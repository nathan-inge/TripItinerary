# tripit Email Worker

Cloudflare Email Worker that receives forwarded confirmation emails on
`trips@in.nathaninge.com`, parses them, HMAC-signs the JSON, and POSTs to
`https://travel.nathaninge.com/api/intake/email`.

## Setup (one-time)

### 1. Add MX records for the intake subdomain

In Cloudflare DNS for `nathaninge.com`, add MX records for `in` pointing at
Cloudflare's email-routing MX servers. The current set is in
[Cloudflare's docs](https://developers.cloudflare.com/email-routing/get-started/enable-email-routing/);
typically:

```
in   MX 10  route1.mx.cloudflare.net
in   MX 20  route2.mx.cloudflare.net
in   MX 50  route3.mx.cloudflare.net
```

### 2. Enable Email Routing on `in.nathaninge.com`

Cloudflare dashboard → Email → Email Routing → Settings → enable for
`in.nathaninge.com`.

### 3. Generate the shared secrets

You need three secrets that live in both the Worker and the backend:

- `TRIPIT_EMAIL_HMAC_KEY` — random 32-byte hex (only the two sides ever see this).
- `CF_ACCESS_CLIENT_ID` + `CF_ACCESS_CLIENT_SECRET` — from a CF Access
  Service Token (Zero Trust → Access → Service Auth → Service Tokens).
  Create a token named `tripit-email-worker`.

```bash
# generate the HMAC key once and stash it
openssl rand -hex 32
```

Put `TRIPIT_EMAIL_HMAC_KEY=...` into the backend's
`~/.config/tripit/env` and restart the service.

### 4. Deploy the Worker

```bash
cd ops/email-worker
npm install
npx wrangler login                                  # if not already
npx wrangler secret put TRIPIT_EMAIL_HMAC_KEY       # paste the same hex
npx wrangler secret put CF_ACCESS_CLIENT_ID         # from CF Access
npx wrangler secret put CF_ACCESS_CLIENT_SECRET     # from CF Access
npx wrangler deploy
```

### 5. Route the address to the Worker

Email Routing → Routing rules → Create rule.
- Match: `trips@in.nathaninge.com`
- Destination: **Send to a Worker** → `tripit-email-worker`

### 6. CF Access policy on `/api/intake/email`

In Zero Trust → Access → Applications, add a third app:

| Field   | Value                                |
|---------|--------------------------------------|
| Domain  | `travel.nathaninge.com`              |
| Path    | `/api/intake/email`                  |

One policy:

| Field   | Value                                                       |
|---------|-------------------------------------------------------------|
| Name    | `Email worker`                                              |
| Action  | **Service Auth**                                            |
| Include | **Service Token** → `tripit-email-worker`                   |

Path-specificity puts this above the hostname-scoped Owner app, so requests
to `/api/intake/email` with the right service token headers are allowed
through; everything else continues to need owner SSO.

## How the forward works (end-user view)

1. The owner adds an allow-listed email on the trip's Manage page.
2. That sender writes a new mail to `trips@in.nathaninge.com` whose body
   starts with the trip name on a line by itself, then forwards (or pastes)
   the confirmation underneath:

   ```
   Japan, autumn 2026

   ---------- Forwarded message ----------
   From: Delta Air Lines <noreply@delta.com>
   ...
   ```

3. The Worker delivers to the backend; the backend matches the trip,
   verifies the sender is allow-listed, parses with Claude, and inserts
   an item on the itinerary.

If the trip name doesn't match or the sender isn't allow-listed, the
backend silently drops the email (HTTP 204, no retry, no error to the
user). Watch the systemd logs (`journalctl --user -u tripit -f`) to
confirm what's being accepted/rejected.

## Local development

You can't easily run the Worker locally against real Email Routing, but
you can simulate the POST by hand:

```bash
KEY=$(cat ~/.config/tripit/env | grep TRIPIT_EMAIL_HMAC_KEY | cut -d= -f2)
BODY='{"from":"nate@gmail.com","subject":"Fwd: test","text":"Japan, autumn 2026\n\nForwarded confirmation goes here...","html":"","received_at":"2026-06-20T12:00:00Z"}'
SIG=$(printf '%s' "$BODY" | openssl dgst -sha256 -hmac "$KEY" -hex | cut -d' ' -f2)
curl -X POST http://127.0.0.1:5050/api/intake/email \
  -H "content-type: application/json" \
  -H "x-tripit-signature: $SIG" \
  -d "$BODY"
```

(Set `DEV_BYPASS_AUTH=1` on the backend or you'll also need the CF Access
service-token headers.)
