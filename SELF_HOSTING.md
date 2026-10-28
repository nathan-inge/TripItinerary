# Self-hosting with Docker

One container, one SQLite file in a volume. Parsing can run fully local
through Ollama, so confirmation emails never leave your machine.

For the original bare-metal setup (systemd + Cloudflare Tunnel + Cloudflare
Access), see [DEPLOY.md](./DEPLOY.md).

## Quick start (local trial)

```bash
git clone <this repo> tripit && cd tripit
cp .env.example .env          # set OWNER_EMAIL; pick a parse backend
docker compose up -d --build
open http://localhost:5050
```

The compose file defaults to `TRIPIT_AUTH_MODE=none`. There's no login, and
every request acts as the first `OWNER_EMAIL`. The port is bound to
`127.0.0.1` only. Don't expose it like this.

### Parsing backend

Pick one in `.env`:

| Backend | `.env` | Notes |
|---|---|---|
| Ollama (local) | `TRIPIT_OLLAMA_URL=http://ollama:11434` | Start with `docker compose --profile ollama up -d`, then `docker compose exec ollama ollama pull qwen2.5:7b-instruct`. Nothing leaves the host. |
| Anthropic | `ANTHROPIC_API_KEY=sk-ant-...` | Better extraction. Text is PII-redacted before it's sent. Capped by `TRIPIT_DAILY_TOKEN_CAP`. |
| None | (neither) | Paste still works; you fill in fields by hand. |

## Auth modes

`TRIPIT_AUTH_MODE` controls how *signed-in* users are identified. Guest share
links (`/share/<token>`) and email intake have their own auth and work the
same in every mode.

| Mode | Use when | How it identifies you |
|---|---|---|
| `none` | Trying it out on your own machine | Everyone is the first `OWNER_EMAIL` |
| `header` | You run a reverse proxy that handles login | Trusts an email header the proxy sets (`TRIPIT_AUTH_HEADER`, default `X-Forwarded-Email`) |
| `cloudflare` | You use Cloudflare Access | Verifies the Access JWT ([DEPLOY.md](./DEPLOY.md)) |

In every mode, an authenticated email only gets in if it's in `OWNER_EMAIL`,
is a member of a trip, or owns a trip. A login proxy that lets in more
people than you intended still doesn't grant them anything.

### `header` mode: the one rule

The app believes whatever the header says. **The proxy must be the only path
to the app, and it must overwrite or strip any client-supplied copy of the
header.** The compose file's `127.0.0.1` port binding takes care of the first
part when the proxy runs on the same host.

The proxy also has to let guests through *without* login on these paths, or
share links break:

```
/share/*  /api/share/*  /api/health  /api/intake/email
/assets/*  /manifest.webmanifest  /sw.js  /workbox-*  /icon.svg
```

None of those paths read the identity header, so leaving them unauthenticated
doesn't open anything else.

#### Example: Caddy + Authelia

```caddyfile
tripit.example.com {
	@public path /share/* /api/share/* /api/health /api/intake/email /assets/* /manifest.webmanifest /sw.js /workbox-* /icon.svg
	handle @public {
		reverse_proxy 127.0.0.1:5050
	}
	handle {
		request_header -Remote-Email # never trust a client-supplied copy
		forward_auth authelia:9091 {
			uri /api/authz/forward-auth
			copy_headers Remote-Email
		}
		reverse_proxy 127.0.0.1:5050
	}
}
```

```bash
# .env
TRIPIT_AUTH_MODE=header
TRIPIT_AUTH_HEADER=Remote-Email
OWNER_EMAIL=you@example.com
```

For oauth2-proxy, use the same shape with `X-Forwarded-Email` (the default
header, so `TRIPIT_AUTH_HEADER` can stay unset).

#### Example: Tailscale Serve

`tailscale serve` adds `Tailscale-User-Login` for people on your tailnet:

```bash
tailscale serve --bg 5050
```

```bash
# .env
TRIPIT_AUTH_MODE=header
TRIPIT_AUTH_HEADER=Tailscale-User-Login
OWNER_EMAIL=you@example.com
```

The catch: guests need to be on your tailnet to open share links. If they
aren't, put a public proxy in front instead.

## Email intake (optional)

Forwarding confirmations by email needs a Cloudflare Email Worker
([DEPLOY.md §7](./DEPLOY.md)). That works with any auth mode, since the intake
endpoint is authenticated by its HMAC signature. Build the image with the
address so the UI shows it:

```bash
VITE_INTAKE_ADDRESS=trips@in.example.com docker compose up -d --build
```

and set `TRIPIT_EMAIL_HMAC_KEY` in `.env`. If the key is unset, the intake
endpoint ignores everything it receives.

## Operations

- **Upgrade:** `git pull && docker compose up -d --build`. Migrations run on
  startup.
- **Backup:** the whole state is `/data/db.sqlite` in the `tripit-data`
  volume. Stop the container and copy the volume, or run
  [Litestream](https://litestream.io) next to it for continuous replication.
- **Logs:** `docker compose logs -f tripit`. Startup logs state the auth mode
  and parse backend in use.
