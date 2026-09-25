import type { ReactNode } from "react";

function Svg({ children, size = 16 }: { children: ReactNode; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className="shrink-0"
    >
      {children}
    </svg>
  );
}

export const RefreshIcon = () => (
  <Svg>
    <path d="M21 12a9 9 0 1 1-2.64-6.36" />
    <path d="M21 3v6h-6" />
  </Svg>
);

export const AlertIcon = () => (
  <Svg>
    <path d="M10.3 3.9 1.8 18.2A2 2 0 0 0 3.5 21h17a2 2 0 0 0 1.7-2.8L13.7 3.9a2 2 0 0 0-3.4 0Z" />
    <path d="M12 9v4M12 17h.01" />
  </Svg>
);

export const CheckIcon = () => (
  <Svg>
    <circle cx="12" cy="12" r="9" />
    <path d="m8.5 12.5 2.5 2.5 4.5-5" />
  </Svg>
);

export const CloseIcon = () => (
  <Svg size={14}>
    <path d="M18 6 6 18M6 6l12 12" />
  </Svg>
);

export const PlusIcon = () => (
  <Svg>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);

export const TrashIcon = () => (
  <Svg>
    <path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6" />
  </Svg>
);

export const EraseIcon = () => (
  <Svg>
    <path d="m7 21-4-4 11-11 7 7-7 7" />
    <path d="M22 21H7M14 6l4 4" />
  </Svg>
);

export const ChevronIcon = ({ open }: { open: boolean }) => (
  <span
    className={`text-(--md-color-text-muted) transition-transform duration-200 ${open ? "rotate-180" : ""}`}
  >
    <Svg>
      <path d="m6 9 6 6 6-6" />
    </Svg>
  </span>
);

export const MoonIcon = ({ size = 16 }: { size?: number }) => (
  <Svg size={size}>
    <path d="M20.5 14.5A8.5 8.5 0 1 1 9.5 3.5a7 7 0 0 0 11 11Z" />
  </Svg>
);

export const DiskIcon = () => (
  <Svg>
    <rect x="3" y="5" width="18" height="14" rx="2" />
    <path d="M3 13h18M7 16h.01M11 16h.01" />
  </Svg>
);

export const UsbIcon = ({ size = 16 }: { size?: number }) => (
  <Svg size={size}>
    <rect x="7" y="9" width="10" height="13" rx="2" />
    <path d="M9 9V3h6v6M11 5.5v.01M13 5.5v.01" />
  </Svg>
);

export const DiscIcon = ({ size = 16 }: { size?: number }) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="9" />
    <circle cx="12" cy="12" r="2.5" />
  </Svg>
);
