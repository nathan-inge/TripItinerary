let reauthing = false;

// Wipes any locally cached app shell and reloads so the next navigation hits
// the network — which, if the Cloudflare Access session is expired, lands on
// the CF login page. Without this, the service worker serves cached index.html
// forever and the user can't get back to login (especially on mobile, where
// there's no hard-refresh gesture).
export function forceReauth(): Promise<never> {
  if (reauthing) return new Promise<never>(() => {});
  reauthing = true;

  return (async () => {
    try {
      if ('serviceWorker' in navigator) {
        const regs = await navigator.serviceWorker.getRegistrations();
        await Promise.all(regs.map((r) => r.unregister()));
      }
      if ('caches' in window) {
        const keys = await caches.keys();
        await Promise.all(keys.map((k) => caches.delete(k)));
      }
    } catch {
      // best-effort cleanup; reload anyway
    }
    window.location.reload();
    return new Promise<never>(() => {});
  })();
}
