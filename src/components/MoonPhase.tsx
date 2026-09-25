import { useId } from "react";

/**
 * A small moon whose lit part grows with `fraction` (0..1) — used to show
 * how much of a disk is allocated to partitions.
 */
export function MoonPhase({ fraction, size = 40 }: { fraction: number; size?: number }) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const f = Math.min(1, Math.max(0, fraction));
  const r = 17;
  // Lit area: the right half of the disc, closed by an elliptical
  // terminator that bulges right for a crescent (f < 0.5) and left for a
  // gibbous moon (f > 0.5) — the shape a real moon phase has.
  const rx = r * Math.abs(1 - 2 * f);
  const lit = `M 20 ${20 - r} A ${r} ${r} 0 0 1 20 ${20 + r} A ${rx} ${r} 0 0 ${
    f > 0.5 ? 1 : 0
  } 20 ${20 - r} Z`;

  return (
    <svg viewBox="0 0 40 40" width={size} height={size} aria-hidden="true" className="shrink-0">
      <defs>
        <radialGradient id={`${id}-lit`} cx="38%" cy="32%" r="78%">
          <stop offset="0%" stopColor="#fdfbff" />
          <stop offset="55%" stopColor="#ddd6ff" />
          <stop offset="100%" stopColor="#a89cf2" />
        </radialGradient>
        <clipPath id={`${id}-clip`}>
          <path d={lit} />
        </clipPath>
        <filter id={`${id}-glow`} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="2.2" />
        </filter>
      </defs>
      <circle cx="20" cy="20" r={r} fill="#b9aefb" opacity={0.25 * f} filter={`url(#${id}-glow)`} />
      <circle
        cx="20"
        cy="20"
        r={r}
        fill="#221b47"
        stroke="rgb(185 174 251 / 0.35)"
        strokeWidth="1"
      />
      <g clipPath={`url(#${id}-clip)`}>
        <circle cx="20" cy="20" r={r} fill={`url(#${id}-lit)`} />
        <circle cx="25" cy="14" r="2.6" fill="#8f82e0" opacity="0.28" />
        <circle cx="29" cy="24" r="1.8" fill="#8f82e0" opacity="0.25" />
        <circle cx="20" cy="27" r="3.2" fill="#8f82e0" opacity="0.2" />
        <circle cx="15" cy="17" r="1.6" fill="#8f82e0" opacity="0.22" />
      </g>
    </svg>
  );
}
