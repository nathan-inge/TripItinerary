// Email address that Cloudflare Email Routing forwards to the
// tripit-email-worker. Override at build time with VITE_INTAKE_ADDRESS,
// otherwise we use the production default.
export const INTAKE_ADDRESS: string =
  (import.meta.env.VITE_INTAKE_ADDRESS as string | undefined) ??
  'travel-confirmation@nathaninge.com';
