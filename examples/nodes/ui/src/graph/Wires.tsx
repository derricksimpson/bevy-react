import { memo } from "react";
import { type P, curve } from "./layout";

/** One link (or a stroke segment), from `a` to `b` in graph units: a
 *  `<wire>` element boxed around its curve, the curve given in the box's
 *  pixels. Primitive props, so an unmoved wire skips re-rendering. */
export const Wire = memo(function Wire({
  ax,
  ay,
  bx,
  by,
  z,
  color,
  colorTo,
  thickness = 2.4,
  dash = 0,
  straight = false,
}: {
  ax: number;
  ay: number;
  bx: number;
  by: number;
  z: number;
  color: string;
  colorTo?: string;
  thickness?: number;
  dash?: number;
  straight?: boolean;
}) {
  const a = { x: ax, y: ay };
  const b = { x: bx, y: by };
  const pts: P[] = straight
    ? [
        a,
        { x: ax + (bx - ax) / 3, y: ay + (by - ay) / 3 },
        { x: ax + ((bx - ax) * 2) / 3, y: ay + ((by - ay) * 2) / 3 },
        b,
      ]
    : curve(a, b);
  const t = Math.max(1.5, thickness * z);
  const pad = t + 3;
  const xs = pts.map((p) => p.x * z);
  const ys = pts.map((p) => p.y * z);
  const left = Math.min(...xs) - pad;
  const top = Math.min(...ys) - pad;
  const local = pts.flatMap((p) => [p.x * z - left, p.y * z - top]) as [
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
  ];
  return (
    <wire
      points={local}
      color={color}
      colorTo={colorTo}
      thickness={t}
      dash={dash * z}
      style={{
        positionType: "absolute",
        left,
        top,
        width: Math.max(...xs) - left + pad,
        height: Math.max(...ys) - top + pad,
      }}
    />
  );
});
