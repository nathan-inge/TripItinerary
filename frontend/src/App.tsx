import { Link, Route, Routes } from 'react-router-dom';
import TripList from './pages/TripList';
import Trip from './pages/Trip';
import TripManage from './pages/TripManage';
import SharedTrip from './pages/SharedTrip';
import Help from './pages/Help';

export default function App() {
  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-10 bg-teal-700 text-white shadow-sm">
        <div className="mx-auto flex max-w-3xl items-center justify-between px-4 py-3">
          <Link to="/" className="text-lg font-semibold tracking-tight">
            TripItinerary
          </Link>
          <Link
            to="/help"
            className="text-xs text-teal-100 hover:text-white"
            title="How parsing works + safeguards"
          >
            ? Help
          </Link>
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
