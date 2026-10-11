/** Circle geometry for the round holograms (stasis, lock, hack). Angles in
 *  degrees, 0 at the top, growing clockwise (screen y points down). */

/** The point at `deg` on the circle of radius `r` around (`cx`, `cy`). */
export function polar(cx: number, cy: number, r: number, deg: number) {
  const a = ((deg - 90) * Math.PI) / 180;
  return [cx + r * Math.cos(a), cy + r * Math.sin(a)] as const;
}

/** An SVG path for the arc of radius `r` from `from` to `to` degrees
 *  (clockwise; `to` > `from`). */
export function arc(
  cx: number,
  cy: number,
  r: number,
  from: number,
  to: number,
): string {
  const [x0, y0] = polar(cx, cy, r, from);
  const [x1, y1] = polar(cx, cy, r, to);
  const large = to - from > 180 ? 1 : 0;
  return `M${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

/** `deg` wrapped into 0..360. */
export function wrap(deg: number): number {
  return ((deg % 360) + 360) % 360;
}

/** The shortest angular distance between two angles, 0..180. */
export function angleBetween(a: number, b: number): number {
  const d = Math.abs(wrap(a) - wrap(b));
  return d > 180 ? 360 - d : d;
}
