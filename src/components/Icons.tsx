import type { ReactNode } from 'react';

interface P {
  size?: number;
  className?: string;
  strokeWidth?: number;
}

function Svg({
  size = 18,
  className,
  children,
  strokeWidth = 1.8,
  fill = 'none',
}: P & { children: ReactNode; fill?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={fill}
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      {children}
    </svg>
  );
}

export const IconPlay = (p: P) => (
  <Svg {...p} fill="currentColor" strokeWidth={0}>
    <path d="M8 5.14v13.72a1 1 0 0 0 1.52.85l11.14-6.86a1 1 0 0 0 0-1.7L9.52 4.29A1 1 0 0 0 8 5.14z" />
  </Svg>
);

export const IconPause = (p: P) => (
  <Svg {...p} fill="currentColor" strokeWidth={0}>
    <rect x="6" y="4" width="4.5" height="16" rx="1.4" />
    <rect x="13.5" y="4" width="4.5" height="16" rx="1.4" />
  </Svg>
);

export const IconPrev = (p: P) => (
  <Svg {...p} fill="currentColor" strokeWidth={0}>
    <path d="M18 5.5v13a1 1 0 0 1-1.55.83L8 13.6V18a1 1 0 0 1-2 0V6a1 1 0 0 1 2 0v4.4l8.45-5.73A1 1 0 0 1 18 5.5z" />
  </Svg>
);

export const IconNext = (p: P) => (
  <Svg {...p} fill="currentColor" strokeWidth={0}>
    <path d="M6 5.5v13a1 1 0 0 0 1.55.83L16 13.6V18a1 1 0 0 0 2 0V6a1 1 0 0 0-2 0v4.4L7.55 4.67A1 1 0 0 0 6 5.5z" />
  </Svg>
);

export const IconVolume = (p: P) => (
  <Svg {...p}>
    <path d="M11 5 6 9H3v6h3l5 4V5z" />
    <path d="M15.5 8.5a4.5 4.5 0 0 1 0 7" />
    <path d="M18 6a8 8 0 0 1 0 12" />
  </Svg>
);

export const IconVolumeMute = (p: P) => (
  <Svg {...p}>
    <path d="M11 5 6 9H3v6h3l5 4V5z" />
    <path d="m16 9 5 6M21 9l-5 6" />
  </Svg>
);

export const IconHeart = ({ filled, ...p }: P & { filled?: boolean }) => (
  <Svg {...p} fill={filled ? 'currentColor' : 'none'}>
    <path d="M20.8 5.6a5.4 5.4 0 0 0-7.6 0L12 6.8l-1.2-1.2a5.4 5.4 0 1 0-7.6 7.6L12 21l8.8-7.8a5.4 5.4 0 0 0 0-7.6z" />
  </Svg>
);

export const IconHome = (p: P) => (
  <Svg {...p}>
    <path d="m3 10.5 9-7 9 7" />
    <path d="M5 9.6V20h14V9.6" />
  </Svg>
);

export const IconSearch = (p: P) => (
  <Svg {...p}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="m16 16 4.5 4.5" />
  </Svg>
);

export const IconLibrary = (p: P) => (
  <Svg {...p}>
    <path d="M4 20V6a2 2 0 0 1 2-2h1v16H6a2 2 0 0 1-2-2z" />
    <path d="M9 4h2v16H9z" />
    <path d="m14 5.5 4.2-1a1 1 0 0 1 1.2.8l2.4 12.9a1 1 0 0 1-.8 1.1l-2 .5" />
  </Svg>
);

export const IconUsers = (p: P) => (
  <Svg {...p}>
    <circle cx="9" cy="8" r="3.4" />
    <path d="M2.5 20a6.5 6.5 0 0 1 13 0" />
    <path d="M16 5.2a3.4 3.4 0 0 1 0 6.6" />
    <path d="M17.5 14.2A6.5 6.5 0 0 1 21.5 20" />
  </Svg>
);

export const IconSettings = (p: P) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1A1.7 1.7 0 0 0 4.6 8a1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V2a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
  </Svg>
);

export const IconRepeat = (p: P) => (
  <Svg {...p}>
    <path d="m17 2 4 4-4 4" />
    <path d="M3 11V9a4 4 0 0 1 4-4h14" />
    <path d="m7 22-4-4 4-4" />
    <path d="M21 13v2a4 4 0 0 1-4 4H3" />
  </Svg>
);

export const IconRepeatOne = (p: P) => (
  <Svg {...p}>
    <path d="m17 2 4 4-4 4" />
    <path d="M3 11V9a4 4 0 0 1 4-4h14" />
    <path d="m7 22-4-4 4-4" />
    <path d="M21 13v2a4 4 0 0 1-4 4H3" />
    <path d="M11 15V9h-2" strokeWidth={2} />
  </Svg>
);

export const IconShuffle = (p: P) => (
  <Svg {...p}>
    <path d="M3 6h4l10 12h4" />
    <path d="m18 15 3 3-3 3" />
    <path d="M3 18h4l2-2.5" />
    <path d="m13 8.5 2-2.5h6" />
    <path d="m18 3 3 3-3 3" />
  </Svg>
);

export const IconList = (p: P) => (
  <Svg {...p}>
    <path d="M8 6h13M8 12h13M8 18h13" />
    <circle cx="4" cy="6" r="1" fill="currentColor" />
    <circle cx="4" cy="12" r="1" fill="currentColor" />
    <circle cx="4" cy="18" r="1" fill="currentColor" />
  </Svg>
);

export const IconMic = (p: P) => (
  <Svg {...p}>
    <rect x="9" y="2.5" width="6" height="11" rx="3" />
    <path d="M5 11a7 7 0 0 0 14 0" />
    <path d="M12 18v3.5" />
  </Svg>
);

export const IconChevronDown = (p: P) => (
  <Svg {...p}>
    <path d="m6 9 6 6 6-6" />
  </Svg>
);

export const IconX = (p: P) => (
  <Svg {...p}>
    <path d="M18 6 6 18M6 6l12 12" />
  </Svg>
);

export const IconPlus = (p: P) => (
  <Svg {...p}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);

export const IconTrash = (p: P) => (
  <Svg {...p}>
    <path d="M4 7h16" />
    <path d="M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
    <path d="M6 7v12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7" />
  </Svg>
);

export const IconCrown = (p: P) => (
  <Svg {...p} fill="currentColor" strokeWidth={0}>
    <path d="M3 18h18l-1.6-8-4.4 4L12 6 9 14 4.6 10z" />
  </Svg>
);

export const IconSend = (p: P) => (
  <Svg {...p}>
    <path d="m3 20 18-8L3 4v6l12 2-12 2z" />
  </Svg>
);

export const IconBili = (p: P) => (
  <Svg {...p}>
    <rect x="2.5" y="6" width="19" height="13" rx="3" />
    <path d="m7 2.5 3 3.5M17 2.5l-3 3.5" />
  </Svg>
);

export const IconMusic = (p: P) => (
  <Svg {...p}>
    <path d="M9 18V5l11-2v13" />
    <circle cx="6.5" cy="18" r="3" />
    <circle cx="17.5" cy="16" r="3" />
  </Svg>
);
