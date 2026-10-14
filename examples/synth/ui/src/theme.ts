import type { BevyStyle } from "bevy-react";
import type { Signal } from "./bevy";

/** `RemSize` at zoom 1 (`REM` in `view.rs`). Every length inside the graph
 *  is written with `r()`: in rem, so Bevy re-lays the graph out at each
 *  zoom and text stays crisp. Chrome outside the graph uses plain px — and
 *  always an explicit `fontSize`, since an unset one is `1rem` and zooms. */
export const REM = 16;

/** A graph length: world px (logical px at zoom 1) in rem. */
export const r = (px: number) => `${+(px / REM).toFixed(4)}rem`;

export const C = {
  text: "#e6e9f2",
  dim: "#8d94a8",
  faint: "#596078",
  card: "#151a26",
  cardDeep: "#0d1018",
  edge: "#262d40",
  edgeHi: "#3a4360",
  glass: "rgba(18, 22, 34, 0.62)",
  glassEdge: "rgba(255, 255, 255, 0.07)",
};

/** Socket and wire colors per signal. */
export const SIGNAL: Record<Signal, string> = {
  audio: "#2ee6c5",
  cv: "#ffb547",
  gate: "#ff5fa2",
};

export const FONT_MONO = "Mono";

/** A frosted floating panel over the graph (the camera's backdrop — the
 *  grid and the wires — blurred behind it). No `boxShadow`: the backdrop
 *  makes the panel a layer, which captures only its border box. */
export const glass: BevyStyle = {
  backgroundColor: C.glass,
  border: 1,
  borderColor: C.glassEdge,
  borderRadius: 14,
  backdropFilter: [
    { name: "blur", params: { radius: 16 } },
    { name: "saturate", params: { amount: 1.4 } },
  ],
  // The graph under a panel never sees its presses.
  focusPolicy: "block",
};

/** `#rrggbb` + alpha (0..1) → `#rrggbbaa`. */
export function alpha(hex: string, a: number): string {
  const byte = Math.round(Math.max(0, Math.min(1, a)) * 255);
  return hex + byte.toString(16).padStart(2, "0");
}
