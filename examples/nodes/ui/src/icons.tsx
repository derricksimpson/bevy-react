import type { BevyStyle } from "bevy-react";

const PATHS = {
  chevronDown: "M3 6 L8 11 L13 6",
  chevronRight: "M6 3 L11 8 L6 13",
  check: "M3 8.5 L6.5 12 L13 4.5",
  plus: "M8 3 V13 M3 8 H13",
  minus: "M3 8 H13",
  close: "M4 4 L12 12 M12 4 L4 12",
  sphere: "M8 1.5 A6.5 6.5 0 1 1 7.99 1.5 M1.5 8 A6.5 2.4 0 0 0 14.5 8",
  cube: "M8 1.5 L14 4.8 L14 11.2 L8 14.5 L2 11.2 L2 4.8 Z M2 4.8 L8 8 L14 4.8 M8 8 V14.5",
  plane: "M1.5 11 L5.5 4.5 H14.5 L10.5 11 Z",
  torus:
    "M8 3.5 C13 3.5 15 6 15 8 C15 10 13 12.5 8 12.5 C3 12.5 1 10 1 8 C1 6 3 3.5 8 3.5 Z M5 8 C5 7 6.3 6.5 8 6.5 C9.7 6.5 11 7 11 8",
  spin: "M13 8 A5 5 0 1 1 11.5 4.5 M11.8 1.8 L11.6 4.7 L14.4 5",
  eye: "M1.5 8 C3.5 4.5 12.5 4.5 14.5 8 C12.5 11.5 3.5 11.5 1.5 8 Z M8 6.3 A1.7 1.7 0 1 1 7.99 6.3",
  grid: "M2 2 H14 V14 H2 Z M6 2 V14 M10 2 V14 M2 6 H14 M2 10 H14",
  search: "M7 2.5 A4.5 4.5 0 1 1 6.99 2.5 M10.3 10.3 L14 14",
} as const;

export type IconName = keyof typeof PATHS;

/** A 16×16 stroked icon. */
export function Icon({
  name,
  size = 12,
  color = "#e6e6e6",
  stroke = 1.5,
  style,
}: {
  name: IconName;
  size?: number;
  color?: string;
  stroke?: number;
  style?: BevyStyle;
}) {
  return (
    <svg
      viewBox="0 0 16 16"
      style={{ width: size, height: size, flexShrink: 0, ...style }}
    >
      <path
        d={PATHS[name]}
        fill="none"
        stroke={color}
        strokeWidth={stroke}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
