import { Link } from 'react-router-dom';
import { INTAKE_ADDRESS } from '../lib/intake';

interface Props {
  tripId: string;
  tripName: string;
  /** Where the "Manage senders" link points. Owner page → `/trips/:id/manage`.
   *  Share guests don't have manage access — pass null to hide the link. */
  manageHref?: string | null;
}

export default function EmailIntakeHint({ tripId, tripName, manageHref }: Props) {
  const href =
    manageHref === undefined ? `/trips/${tripId}/manage` : manageHref;

  const copy = (text: string) => navigator.clipboard?.writeText(text);

  // Email intake is optional; hide this when it isn't configured.
  const address = INTAKE_ADDRESS;
  if (!address) return null;

  return (
    <div className="rounded-lg border border-slate-200 bg-slate-50 p-3 text-xs text-slate-600">
      <p>
        <span aria-hidden>📧 </span>
        Or forward a confirmation to{' '}
        <button
          type="button"
          onClick={() => copy(address)}
          title="Copy address"
          className="rounded bg-white px-1.5 py-0.5 font-mono text-[11px] text-slate-800 hover:bg-slate-100"
        >
          {address}
        </button>{' '}
        with{' '}
        <button
          type="button"
          onClick={() => copy(tripName)}
          title="Copy trip name"
          className="rounded bg-white px-1.5 py-0.5 font-mono text-[11px] text-slate-800 hover:bg-slate-100"
        >
          {tripName}
        </button>{' '}
        as the first line of the body.
        {href && (
          <>
            {' '}
            <Link
              to={href}
              className="text-teal-700 hover:underline"
            >
              Manage senders →
            </Link>
          </>
        )}{' '}
        <Link to="/help" className="text-teal-700 hover:underline">
          How parsing works →
        </Link>
      </p>
    </div>
  );
}
