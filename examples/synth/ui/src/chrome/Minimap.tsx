import { useRef } from "react";
import type { PointerEventData } from "bevy-react";
import type { WindowSize } from "../bevy";
import { CATEGORIES, kind } from "../graph/catalog";
import { type Graph, bounds, live, nodeBox } from "../graph/model";
import type { View } from "../graph/Canvas";
import { C, alpha, glass } from "../theme";

const W = 208;
const H = 132;

/** The whole patch in a corner: press or drag to fly the view there. One
 *  svg in world coordinates — the modules, their wires, the view's outline —
 *  framed by its `viewBox`, so a pan or a zoom changes the frame and the
 *  outline, not every module. */
export function Minimap({
  graph,
  view,
  size,
  onCenter,
}: {
  graph: Graph;
  view: View;
  size: WindowSize;
  /** Center the view on a world point; `glide` ms. */
  onCenter: (x: number, y: number, glide: number) => void;
}) {
  const nodes = live(graph);
  const seen = {
    x: -view.x / view.zoom,
    y: -view.y / view.zoom,
    w: size.width / view.zoom,
    h: size.height / view.zoom,
  };
  const b = bounds(nodes) ?? seen;
  const pad = 80;
  const x0 = Math.min(b.x, seen.x) - pad;
  const y0 = Math.min(b.y, seen.y) - pad;
  const x1 = Math.max(b.x + b.w, seen.x + seen.w) + pad;
  const y1 = Math.max(b.y + b.h, seen.y + seen.h) + pad;
  const s = Math.min(W / (x1 - x0), H / (y1 - y0));
  const ox = (W - (x1 - x0) * s) / 2 - x0 * s;
  const oy = (H - (y1 - y0) * s) / 2 - y0 * s;
  // One map pixel, in world units.
  const px = 1 / s;

  // The scale is frozen while dragging, or the map would slide under the hand.
  const frozen = useRef<{ s: number; ox: number; oy: number } | null>(null);
  const fly = (e: PointerEventData, glide: number) => {
    const m = frozen.current ?? { s, ox, oy };
    onCenter((e.x * W - m.ox) / m.s, (e.y * H - m.oy) / m.s, glide);
  };

  let wires = "";
  for (const l of graph.links) {
    const a = graph.nodes[l.from.node];
    const z = graph.nodes[l.to.node];
    if (!a || !z) continue;
    const [ax, ay] = [a.x + kind(a.kind).width, a.y + 50];
    const [zx, zy] = [z.x, z.y + 50];
    const mid = Math.round((ax + zx) / 2);
    wires += `M${ax} ${ay}C${mid} ${ay} ${mid} ${zy} ${zx} ${zy}`;
  }

  return (
    <node
      onPointerDown={(e) => {
        frozen.current = { s, ox, oy };
        fly(e, 260);
      }}
      onPointerMove={(e) => fly(e, 0)}
      onPointerUp={() => {
        frozen.current = null;
      }}
      style={{
        ...glass,
        positionType: "absolute",
        right: 14,
        bottom: 14,
        width: W + 2,
        height: H + 2,
        overflowX: "clip",
        overflowY: "clip",
        cursor: "pointer",
      }}
    >
      <svg
        viewBox={`${-ox * px} ${-oy * px} ${W * px} ${H * px}`}
        style={{
          positionType: "absolute",
          left: 0,
          top: 0,
          width: W,
          height: H,
        }}
      >
        <path
          d={wires}
          fill="none"
          stroke={alpha("#ffffff", 0.22)}
          strokeWidth={px}
        />
        {nodes.map((n) => {
          const r = nodeBox(n);
          const color = CATEGORIES[kind(n.kind).category].color;
          const sel = graph.selected.includes(n.id);
          return (
            <rect
              key={n.id}
              x={r.x}
              y={r.y}
              width={r.w}
              height={r.h}
              // World units: about 2 map px, and it never changes with the
              // frame.
              rx={30}
              fill={alpha(color, sel ? 0.9 : 0.45)}
            />
          );
        })}
        <rect
          x={seen.x}
          y={seen.y}
          width={seen.w}
          height={seen.h}
          rx={3 * px}
          fill={alpha(C.text, 0.04)}
          stroke={alpha(C.text, 0.55)}
          strokeWidth={px}
        />
      </svg>
    </node>
  );
}
