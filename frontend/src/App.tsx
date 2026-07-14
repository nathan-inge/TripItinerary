import { useEffect, useState } from 'react';
import { Link, Route, Routes, useLocation } from 'react-router-dom';
import { api } from './api/client';
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
  const [email, setEmail] = useState<string | null>(null);

  useEffect(() => {
    if (isShare) return;
    let cancelled = false;
    api
      .getMe()
      .then((me) => {
        if (!cancelled) setEmail(me.email);
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
          {!isShare && email && (
            <span
              className="max-w-[55%] truncate text-xs text-teal-100"
              title={`Signed in as ${email}`}
            >
              {email}
            </span>
          )}
        </div>
      </header>
      <main className="mx-auto max-w-3xl px-4 py-6">
        <Routes>
          <Route path="/" element={<TripList />} />
          <Route path="/trips/:id" element={<Trip />} />
          <Route path="/trips/:id/manage" element={<TripManage />} />
          <Route path="/share/:token" element={<SharedTrip />} />
          <Route path="/help" element={<Help />} />
        </Routes>
      </main>
    </div>
  );
}
