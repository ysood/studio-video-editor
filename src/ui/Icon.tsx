// One icon set for the whole app, drawn on a 16-unit grid with the current text color.
// Restyling icons (stroke weight, swapping the set) happens here and in the theme only.

import type { SVGProps } from "react";
import { cn } from "../lib/cn";

const paths = {
  play: <path d="M5 3.5v9l7-4.5z" fill="currentColor" stroke="none" />,
  pause: (
    <>
      <rect x="4" y="3.5" width="2.6" height="9" rx="0.6" fill="currentColor" stroke="none" />
      <rect x="9.4" y="3.5" width="2.6" height="9" rx="0.6" fill="currentColor" stroke="none" />
    </>
  ),
  toStart: <path d="M4 3.5v9M12 3.5v9L6 8z" />,
  toEnd: <path d="M12 3.5v9M4 3.5v9L10 8z" />,
  stepBack: <path d="M10.5 3.5L5.5 8l5 4.5" />,
  stepForward: <path d="M5.5 3.5l5 4.5-5 4.5" />,
  eye: (
    <>
      <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z" />
      <circle cx="8" cy="8" r="2" />
    </>
  ),
  eyeOff: (
    <>
      <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z" />
      <path d="M2.5 13.5l11-11" />
    </>
  ),
  speaker: (
    <>
      <path d="M2.5 6h2.5L8.5 3v10L5 10H2.5Z" />
      <path d="M11 5.5a3.5 3.5 0 0 1 0 5" />
    </>
  ),
  mute: (
    <>
      <path d="M2.5 6h2.5L8.5 3v10L5 10H2.5Z" />
      <path d="M10.5 6l3.5 4M14 6l-3.5 4" />
    </>
  ),
  lock: (
    <>
      <rect x="3" y="7.5" width="10" height="6.5" rx="1" />
      <path d="M5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5" />
    </>
  ),
  unlock: (
    <>
      <rect x="3" y="7.5" width="10" height="6.5" rx="1" />
      <path d="M5.5 7.5V5a2.5 2.5 0 0 1 4.9-.7" />
    </>
  ),
  close: <path d="M4 4l8 8M12 4l-8 8" />,
  video: (
    <>
      <rect x="2" y="4" width="8.5" height="8" rx="1" />
      <path d="M10.5 7l3.5-2v6l-3.5-2" />
    </>
  ),
  audio: <path d="M6 12.5V4l6.5-1.5V11M6 12.5a1.8 1.8 0 1 1-1.8-1.8A1.8 1.8 0 0 1 6 12.5Zm6.5-1.5a1.8 1.8 0 1 1-1.8-1.8 1.8 1.8 0 0 1 1.8 1.8Z" />,
  image: (
    <>
      <rect x="2" y="3" width="12" height="10" rx="1" />
      <path d="M2.5 11l3.5-3.5 3 3 2-2 2.5 2.5" />
      <circle cx="10.5" cy="6" r="1" />
    </>
  ),
  plus: <path d="M8 3v10M3 8h10" />,
  chevronDown: <path d="M4 6l4 4 4-4" />,
  check: <path d="M3.5 8.5l3 3 6-7" />,
  more: (
    <>
      <circle cx="3.5" cy="8" r="0.9" fill="currentColor" />
      <circle cx="8" cy="8" r="0.9" fill="currentColor" />
      <circle cx="12.5" cy="8" r="0.9" fill="currentColor" />
    </>
  ),
  scissors: (
    <>
      <circle cx="4.5" cy="11.5" r="2" />
      <circle cx="11.5" cy="11.5" r="2" />
      <path d="M6 10L12.5 2.5M10 10L3.5 2.5" />
    </>
  ),
  trash: <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 8.5h5.6l.7-8.5" />,
  pencil: <path d="M11 2.5l2.5 2.5-8 8H3v-2.5zM9 4.5l2.5 2.5" />,
  arrowRight: <path d="M3 8h10M9.5 4.5L13 8l-3.5 3.5" />,
  unlink: <path d="M6.5 9.5l3-3M5 7L3.8 8.2a2.5 2.5 0 0 0 3.5 3.5L8.5 10.5M11 9l1.2-1.2a2.5 2.5 0 0 0-3.5-3.5L7.5 5.5M2.5 2.5l2 2M11.5 11.5l2 2" />,
  magnet: <path d="M3.5 3v5a4.5 4.5 0 0 0 9 0V3h-3v5a1.5 1.5 0 0 1-3 0V3zM3.5 5.5h3M9.5 5.5h3" />,
  fit: <path d="M2.5 6V3h3M10.5 3h3v3M13.5 10v3h-3M5.5 13h-3v-3" />,
  undo: <path d="M5.5 4L2.5 7l3 3M2.5 7h7a3.5 3.5 0 0 1 0 7H7" />,
  redo: <path d="M10.5 4l3 3-3 3M13.5 7h-7a3.5 3.5 0 0 0 0 7H9" />,
  export: <path d="M8 10V2.5M5 5.5l3-3 3 3M3 9.5v3.5h10V9.5" />,
  file: <path d="M4 2.5h5l3 3v8H4zM9 2.5v3h3" />,
  grid: (
    <>
      <rect x="2.5" y="2.5" width="4.5" height="4.5" rx="0.6" />
      <rect x="9" y="2.5" width="4.5" height="4.5" rx="0.6" />
      <rect x="2.5" y="9" width="4.5" height="4.5" rx="0.6" />
      <rect x="9" y="9" width="4.5" height="4.5" rx="0.6" />
    </>
  ),
  list: <path d="M5.5 4h8M5.5 8h8M5.5 12h8M2.5 4h0.5M2.5 8h0.5M2.5 12h0.5" />,
  sort: <path d="M2.5 4h11M2.5 8h7M2.5 12h3.5" />,
  timeline: (
    <>
      <rect x="2" y="5" width="12" height="6.5" rx="1" />
      <path d="M5.5 2.5v11" />
    </>
  ),
  folder: <path d="M2 4.5a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z" />,
  stack: <path d="M8 2.5l5.5 3L8 8.5l-5.5-3zM2.5 8.5L8 11.5l5.5-3M2.5 11L8 14l5.5-3" />,
  warning: <path d="M8 2.5l6 11H2zM8 6.5v3.5M8 12h0" />,
  chat: <path d="M2.5 3.5h11v7.5h-6l-3 2.5v-2.5h-2z" />,
  send: <path d="M2.5 8h9M8 4.5L11.5 8 8 11.5" />,
  search: (
    <>
      <circle cx="7" cy="7" r="4.5" />
      <path d="M10.3 10.3l3.2 3.2" />
    </>
  ),
} as const;

export type IconName = keyof typeof paths;

export const iconNames = Object.keys(paths) as IconName[];

export function Icon({ name, className, ...rest }: { name: IconName } & SVGProps<SVGSVGElement>) {
  return (
    <svg
      className={cn("size-icon icon-stroke shrink-0", className)}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable={false}
      {...rest}
    >
      {paths[name]}
    </svg>
  );
}
