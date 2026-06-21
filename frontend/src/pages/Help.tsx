import { Link } from 'react-router-dom';
import { INTAKE_ADDRESS } from '../lib/intake';

export default function Help() {
  return (
    <div className="space-y-6">
      <div>
        <Link to="/" className="text-sm text-teal-700 hover:underline">
          ← Trips
        </Link>
        <h1 className="text-2xl font-bold">How parsing works</h1>
        <p className="text-sm text-slate-600">
          Read this before forwarding confirmations or asking someone else to.
        </p>
      </div>

      <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
        <h2 className="text-lg font-semibold">The path your email takes</h2>
        <ol className="list-decimal space-y-1 pl-5 text-sm text-slate-700">
          <li>You paste a confirmation, or forward one to <code className="rounded bg-slate-100 px-1 font-mono text-[12px]">{INTAKE_ADDRESS}</code>.</li>
          <li>It arrives on the self-hosted server.</li>
          <li>The <b>redactor</b> runs locally — see below for what it strips.</li>
          <li>The redacted text is sent to a language model to extract structured fields.</li>
          <li>The structured item appears on your itinerary. The original is also kept locally so you can re-parse or look up what came in.</li>
        </ol>
      </section>

      <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
        <h2 className="text-lg font-semibold">What gets stripped before the LLM sees it</h2>
        <p className="text-sm text-slate-700">
          The redactor runs <i>before</i> any text leaves the host. These are
          replaced with placeholders:
        </p>
        <ul className="list-disc space-y-1 pl-5 text-sm text-slate-700">
          <li><b>Payment card numbers</b> — Luhn-validated so flight numbers aren't false-positively caught.</li>
          <li><b>Social security numbers</b> — US format.</li>
          <li><b>Phone numbers</b> — including those embedded in <code>tel:</code>, <code>sms:</code>, <code>wa.me/</code>, and WhatsApp URLs.</li>
          <li><b>Email addresses</b> — only those that aren't inside a URL.</li>
          <li><b>Traveler names</b> from your trip's traveler list. Multi-word entries strip each word; longer variants like <i>Nathaniel</i> are caught when <i>Nathan</i> is in the list.</li>
          <li><b>Booking confirmation numbers</b> — detected by labels (<code>Booking</code>, <code>Confirmation</code>, <code>Reservation</code>, <code>PNR</code>, <code>Reference</code>, <code>PIN</code>). The real value is held in a local map and substituted back into the saved item after the LLM responds — so the LLM never sees the real number.</li>
        </ul>
        <p className="text-xs text-slate-500">
          Invisible Unicode separators (zero-width joiners) that some templates
          embed for anti-scraping are stripped before regex matching.
        </p>
      </section>

      <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
        <h2 className="text-lg font-semibold">Where parsing happens</h2>
        <p className="text-sm text-slate-700">
          Set via <code>TRIPIT_PARSE_BACKEND</code> on the server. The options:
        </p>
        <ul className="list-disc space-y-1 pl-5 text-sm text-slate-700">
          <li><b>Anthropic Claude</b> — the redacted text crosses the network to the Anthropic API. Their commercial terms specify customer content is not used for training and is deleted within 30 days. Fast, accurate, hardware-cheap. Comes with the modest residual risk of "a temporary copy exists at Anthropic."</li>
          <li><b>Local Ollama</b> — a model runs on the same host. Nothing leaves. Needs sufficient RAM (qwen2.5:7b-instruct wants ~8 GB host RAM as a practical floor).</li>
          <li><b>Dry-run</b> — skips the LLM call entirely. Useful for verifying the redactor before going live. Logs everything that would have been sent.</li>
        </ul>
      </section>

      <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
        <h2 className="text-lg font-semibold">Audit what was sent</h2>
        <p className="text-sm text-slate-700">
          Every parse logs the redacted body and a per-category placeholder
          count to the host's system journal:
        </p>
        <pre className="overflow-x-auto rounded-lg bg-slate-900 px-3 py-2 text-xs text-slate-100">
          {`journalctl --user -u tripit -f | grep -A 200 'LLM INPUT'`}
        </pre>
        <p className="text-sm text-slate-700">
          You can confirm names, conf #s, cards, and phones are gone before they
          reach the model. If something slips through, the line includes a hint
          (e.g., <code>parser returned a raw conf value</code>) so you can
          add a missing label pattern.
        </p>
      </section>

      <section className="space-y-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
        <h2 className="text-lg font-semibold">Get the best coverage</h2>
        <ul className="list-disc space-y-1 pl-5 text-sm text-slate-700">
          <li>Add your <b>full name</b> (first + last) to each trip's traveler list. Last names matter as much as first names — they're what an attacker would pair with a PNR.</li>
          <li>For people travelling with you: add each as a separate traveler entry so their names get the same redaction coverage.</li>
          <li>Before relying on Anthropic, set <code>TRIPIT_PARSE_BACKEND=dry-run</code> once and forward a real confirmation. The full <code>=== DRY RUN ===</code> log shows exactly what would have been sent.</li>
          <li>The email intake address only accepts forwards from senders in the trip's <b>allow list</b> (Manage page). Don't share the address publicly.</li>
        </ul>
      </section>

      <section className="space-y-3 rounded-xl border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
        <h2 className="text-lg font-semibold">What can still go wrong</h2>
        <ul className="list-disc space-y-1 pl-5">
          <li>A name <i>not in the trip's traveler list</i> will appear in the LLM input. Add it.</li>
          <li>Confirmation #s in unusual formats (without a labeled prefix) can slip past the regex. The server logs a WARN; let the maintainer know which label needs to be added.</li>
          <li>Smaller language models occasionally misread dates or merge fields. Review items on the timeline and tap to edit if something is wrong.</li>
          <li>The forwarded email's headers (<code>From:</code>, <code>To:</code>) reach the host. We only authenticate on <code>From:</code> being on the allow list.</li>
        </ul>
      </section>
    </div>
  );
}
