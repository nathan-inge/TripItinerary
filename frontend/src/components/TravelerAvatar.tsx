/** Deterministic palette indexed by name hash. Tailwind needs literal class
 *  strings so the JIT doesn't tree-shake these. */
const PALETTE = [
  'bg-rose-500',
  'bg-orange-500',
  'bg-amber-500',
  'bg-emerald-500',
  'bg-teal-500',
  'bg-sky-500',
  'bg-indigo-500',
  'bg-fuchsia-500',
] as const;

function hash(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h);
}

function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return '?';
  if (parts.length === 1) return parts[0].charAt(0).toUpperCase();
  return (parts[0].charAt(0) + parts[parts.length - 1].charAt(0)).toUpperCase();
}

interface Props {
  name: string;
  size?: 'sm' | 'md';
}

export function TravelerAvatar({ name, size = 'sm' }: Props) {
  const color = PALETTE[hash(name) % PALETTE.length];
  const sz =
    size === 'md'
      ? 'h-7 w-7 text-xs'
      : 'h-5 w-5 text-[10px]';
  return (
    <span
      title={name}
      aria-label={name}
      className={`${color} ${sz} inline-flex shrink-0 items-center justify-center rounded-full font-semibold text-white ring-1 ring-white`}
    >
      {initials(name)}
    </span>
  );
}

export function TravelerAvatarStack({
  names,
  size = 'sm',
}: {
  names: string[];
  size?: 'sm' | 'md';
}) {
  if (names.length === 0) return null;
  const overlap = size === 'md' ? '-ml-2' : '-ml-1.5';
  return (
    <span className="inline-flex items-center">
      {names.map((n, i) => (
        <span
          key={`${n}-${i}`}
          className={i === 0 ? '' : overlap}
        >
          <TravelerAvatar name={n} size={size} />
        </span>
      ))}
    </span>
  );
}
