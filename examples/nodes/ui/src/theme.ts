import type { Category, SocketType } from "./bevy";

/** The palette: a dark editor in the spirit of Blender's default theme. */
export const C = {
  window: "#1c1c1c",
  bar: "#232323",
  pane: "#2a2a2a",
  paneHeader: "#303030",
  border: "#151515",
  graph: "#1d1d1d",
  grid: "#3c3c3c",
  node: "#303030",
  nodeBorder: "#161616",
  selected: "#ed8f2f",
  active: "#ffffff",
  text: "#e6e6e6",
  textDim: "#a6a6a6",
  textMuted: "#717171",
  field: "#545454",
  fieldHover: "#626262",
  fieldDark: "#1b1b1b",
  fill: "#4772b3",
  fillHover: "#5482c6",
  menu: "#181818",
  menuBorder: "#0b0b0b",
  hover: "#4772b3",
  accent: "#4772b3",
  muted: "#8c2a2a",
  knife: "#ff5555",
  box: "rgba(255, 255, 255, 0.06)",
  boxBorder: "#d0d0d0",
} as const;

/** Node header color and Add-menu title per category. */
export const CATEGORY: Record<Category, { header: string; label: string }> = {
  input: { header: "#86344d", label: "Input" },
  texture: { header: "#80491d", label: "Texture" },
  color: { header: "#6d6826", label: "Color" },
  converter: { header: "#2a6386", label: "Converter" },
  filter: { header: "#53407e", label: "Filter" },
  vector: { header: "#3a3a8a", label: "Vector" },
  layout: { header: "#424242", label: "Layout" },
  output: { header: "#6e2626", label: "Output" },
};

/** The order of the Add menu's categories. */
export const CATEGORIES: Category[] = [
  "input",
  "texture",
  "color",
  "converter",
  "filter",
  "vector",
  "layout",
  "output",
];

export const SOCKET: Record<SocketType, string> = {
  gray: "#a1a1a1",
  color: "#c7c729",
  vector: "#6363c7",
  any: "#8a8a8a",
};

/** Sizes in graph units (multiplied by the zoom inside the graph). */
export const G = {
  nodeWidth: 160,
  outputWidth: 200,
  header: 26,
  row: 22,
  pad: 6,
  thumb: 128,
  ramp: 64,
  reroute: 16,
  socket: 10,
  radius: 6,
  font: 11.5,
} as const;

/** Fixed UI sizes (logical px, outside the graph). */
export const UI = {
  bar: 30,
  paneHeader: 26,
  sidebar: 250,
  font: 12,
  small: 11,
} as const;

/** `#rrggbb` from display values `0..1`. */
export function hex([r, g, b]: readonly number[]): string {
  const c = (v: number) =>
    Math.round(Math.max(0, Math.min(1, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

/** Display values `0..1` from `#rgb`/`#rrggbb`; `null` if it isn't one. */
export function parseHex(text: string): [number, number, number] | null {
  let s = text.trim().replace(/^#/, "");
  if (s.length === 3) s = [...s].map((c) => c + c).join("");
  if (!/^[0-9a-fA-F]{6}$/.test(s)) return null;
  const v = (i: number) => parseInt(s.slice(i, i + 2), 16) / 255;
  return [v(0), v(2), v(4)];
}
