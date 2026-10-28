// Cloudflare Email Worker for tripit.
//
// Trigger: messages routed via Cloudflare Email Routing to this Worker.
// Action:  parse the email body, sign with HMAC-SHA256, and POST to the
// tripit backend's /api/intake/email endpoint.
//
// The backend handles trip-matching (first non-empty line of the body) and
// the per-trip sender allow-list. This Worker is intentionally dumb — it
// doesn't know what trips exist or who's allowed to send.

import PostalMime from 'postal-mime';

interface Env {
  TARGET_URL: string;
  TRIPIT_EMAIL_HMAC_KEY: string;
  CF_ACCESS_CLIENT_ID: string;
  CF_ACCESS_CLIENT_SECRET: string;
}

interface ForwardedJson {
  from: string;
  subject: string;
  text: string;
  html: string;
  received_at: string;
}

export default {
  async email(message: ForwardableEmailMessage, env: Env): Promise<void> {
    // Read the full RFC822 payload. ReadableStream → Uint8Array.
    if (!env.TARGET_URL) {
      console.error('tripit: TARGET_URL secret is not set; dropping email');
      return;
    }

    const raw = await streamToUint8Array(message.raw);

    const parser = new PostalMime();
    const parsed = await parser.parse(raw);

    const body: ForwardedJson = {
      from: message.from,
      subject: parsed.subject ?? '',
      text: parsed.text ?? '',
      html: parsed.html ?? '',
      received_at: new Date().toISOString(),
    };

    const payload = JSON.stringify(body);
    const signature = await hmacSha256Hex(env.TRIPIT_EMAIL_HMAC_KEY, payload);

    const resp = await fetch(env.TARGET_URL, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'x-tripit-signature': signature,
        'cf-access-client-id': env.CF_ACCESS_CLIENT_ID,
        'cf-access-client-secret': env.CF_ACCESS_CLIENT_SECRET,
      },
      body: payload,
    });

    if (!resp.ok) {
      // Never throw — that'd queue a retry, and we don't want repeated
      // attempts to push the same email through the parser.
      console.error('tripit intake POST failed', resp.status, await resp.text());
    }
  },
};

async function streamToUint8Array(stream: ReadableStream<Uint8Array>): Promise<Uint8Array> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    total += value.byteLength;
  }
  const out = new Uint8Array(total);
  let offset = 0;
  for (const c of chunks) {
    out.set(c, offset);
    offset += c.byteLength;
  }
  return out;
}

async function hmacSha256Hex(key: string, message: string): Promise<string> {
  const enc = new TextEncoder();
  const cryptoKey = await crypto.subtle.importKey(
    'raw',
    enc.encode(key),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const sig = await crypto.subtle.sign('HMAC', cryptoKey, enc.encode(message));
  return [...new Uint8Array(sig)]
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}
