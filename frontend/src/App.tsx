import { useEffect, useState } from 'react';
import { Link, Route, Routes, useLocation } from 'react-router-dom';
import { api } from './api/client';
import type { Me } from './api/types';
import TripList from './pages/TripList';
import Trip from './pages/Trip';
import TripManage from './pages/TripManage';
import SharedTrip from './pages/SharedTrip';
import Help from './pages/Help';

export default function App() {
  const location = useLocation();
  // Guest /share/:token routes are unauthenticated; hitting /api/me there would
  // trip the owner reauth-redirect. Only look up the owner on owner routes.
  const isShare = location.pathname.startsWith('/share/');
  const [me, setMe] = useState<Me | null>(null);

  useEffect(() => {
    if (isShare) return;
    let cancelled = false;
    api
      .getMe()
      .then((m) => {
        if (!cancelled) setMe(m);
      })
      .catch(() => {
        // Non-fatal: the header just omits the email. The api client already
        // handles expired-session redirects; anything else we quietly ignore.
      });
    return () => {
      cancelled = true;
    };
  }, [isShare]);

  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-10 bg-teal-700 text-white shadow-sm">
        <div className="mx-auto flex max-w-3xl items-center justify-between px-4 py-3">
          <Link to="/" className="text-lg font-semibold tracking-tight">
            TripItinerary
          </Link>
          {!isShare && me && (
            <span
              className="max-w-[55%] truncate text-xs text-teal-100"
              title={`Signed in as ${me.email}`}
            >
              {me.email}
            </span>
          )}
        </div>
      </header>
      <main className="mx-auto max-w-3xl px-4 py-6">
        {!isShare && me && !me.provisioned ? (
          <NoAccess email={me.email} />
        ) : (
          <Routes>
            <Route path="/" element={<TripList />} />
            <Route path="/trips/:id" element={<Trip />} />
            <Route path="/trips/:id/manage" element={<TripManage />} />
            <Route path="/share/:token" element={<SharedTrip />} />
            <Route path="/help" element={<Help />} />
          </Routes>
        )}
      </main>
    </div>
  );
}

/**
 * Signed in through Cloudflare Access, but nothing has been shared with this
 * address yet — every data route would 403. Say so instead of showing a wall
 * of errors.
 */
function NoAccess({ email }: { email: string }) {
  return (
    <div className="space-y-2 rounded-xl border border-amber-200 bg-amber-50 p-4">
      <h1 className="text-lg font-semibold text-amber-900">No trips yet</h1>
      <p className="text-sm text-amber-800">
        You're signed in as{' '}
        <span className="font-mono text-xs">{email}</span>, but no trips have
        been shared with this address.
      </p>
      <p className="text-sm text-amber-800">
        Ask the trip owner to add this exact address under{' '}
        <strong>Manage → Share with people</strong>. If they sent you a share
        link instead, open that link directly — it doesn't need an account.
      </p>
    </div>
  );
}
