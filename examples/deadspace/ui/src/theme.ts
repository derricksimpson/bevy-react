import type { BevyStyle } from "bevy-react";

/** The suit's interface: cyan light on blue-black glass, white for what is
 *  selected, amber for prices, red for damage and locks, green for what is
 *  open. Holograms are drawn as light (`hologram.wgsl` raises them past 1.0
 *  so they bloom): keep fills dark and translucent, lines and type bright. */
export const C = {
  cyan: "#7fe3ff",
  cyanHi: "#c4f6ff",
  cyanDim: "#3f93ad",
  cyanDeep: "#123a4a",
  cyanFaint: "rgba(127, 227, 255, 0.14)",
  cyanLine: "rgba(127, 227, 255, 0.45)",
  white: "#f0fdff",
  amber: "#ffb347",
  amberDim: "#a8661c",
  red: "#ff4a3d",
  redDim: "#a3261d",
  redFaint: "rgba(255, 74, 61, 0.18)",
  green: "#6dff9a",
  greenDim: "#2f9a55",
  // Fills (dark glass: they dim the room behind a hologram).
  glass: "rgba(4, 18, 28, 0.62)",
  glassHi: "rgba(10, 44, 62, 0.72)",
  shade: "rgba(1, 6, 10, 0.8)",
  clear: "rgba(0, 0, 0, 0)",
};

/** Saira Semi Condensed Medium is the default font; its other weights are
 *  families of their own (static files). Michroma is the display face. */
export const F = {
  regular: "Saira",
  semibold: "Saira SemiBold",
  bold: "Saira Bold",
  display: "Michroma",
};

/** Text styles. */
export const T = {
  /** Hologram headers: wide, spaced capitals. */
  header: {
    fontFamily: F.display,
    fontSize: 22,
    color: C.cyan,
    letterSpacing: 2,
    lineBreak: "noWrap",
  } as BevyStyle,
  /** Tabs, buttons, list rows. */
  label: {
    fontFamily: F.semibold,
    fontSize: 20,
    color: C.cyan,
    letterSpacing: 1,
    lineBreak: "noWrap",
  } as BevyStyle,
  /** Body copy. */
  body: {
    fontFamily: F.regular,
    fontSize: 18,
    color: C.cyan,
    lineHeight: 1.35,
  } as BevyStyle,
  /** Small print: hints, captions, stat names. */
  small: {
    fontFamily: F.regular,
    fontSize: 14,
    color: C.cyanDim,
    letterSpacing: 1,
  } as BevyStyle,
  /** Big numbers (ammo, credits, timers). */
  number: {
    fontFamily: F.display,
    fontSize: 40,
    color: C.cyan,
    lineBreak: "noWrap",
  } as BevyStyle,
};

/** The thin bright frame every hologram panel wears. */
export const frame: BevyStyle = {
  border: 1,
  borderColor: C.cyanLine,
  backgroundColor: C.glass,
};
