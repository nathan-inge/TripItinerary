// Email address that Cloudflare Email Routing forwards to the
// tripit-email-worker. Set at build time with VITE_INTAKE_ADDRESS (e.g. in
// frontend/.env.local). Unset means email intake isn't configured, and the
// UI hides the forwarding hint.
export const INTAKE_ADDRESS: string | undefined =
  (import.meta.env.VITE_INTAKE_ADDRESS as string | undefined) || undefined;
