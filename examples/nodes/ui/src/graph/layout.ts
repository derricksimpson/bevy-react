import type { NodeDef } from "../bevy";
import { def } from "../catalog";
import { G } from "../theme";
import { type Doc, type GNode, type NodeId, linkKey, nodeList } from "./model";

export interface P {
  x: number;
  y: number;
}

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A node's size and its sockets' heights, in graph units. `NodeCard`
 *  renders with exactly these row heights, so wires meet the dots. */
export interface Geometry {
  w: number;
  h: number;
  inputs: Map<string, number>;
  outputs: Map<string, number>;
  /** Top of the thumbnail, when shown. */
  preview: number | null;
}

export function geometry(node: GNode, d: NodeDef = def(node.kind)): Geometry {
  if (node.kind === "reroute") {
    const r = G.reroute;
    return {
      w: r,
      h: r,
      inputs: new Map([["in", r / 2]]),
      outputs: new Map([["out", r / 2]]),
      preview: null,
    };
  }
  const w = d.category === "output" ? G.outputWidth : G.nodeWidth;
  if (node.collapsed) {
    const mid = G.header / 2;
    return {
      w,
      h: G.header,
      inputs: new Map(d.inputs.map((s) => [s.id, mid])),
      outputs: new Map(d.outputs.map((s) => [s.id, mid])),
      preview: null,
    };
  }
  let y = G.header + G.pad / 2;
  const outputs = new Map<string, number>();
  for (const s of d.outputs) {
    outputs.set(s.id, y + G.row / 2);
    y += G.row;
  }
  let preview: number | null = null;
  if (d.preview && !node.hidePreview) {
    preview = y + G.pad / 2;
    y += G.thumb + G.pad;
  }
  for (const p of d.params) y += p.field.kind === "ramp" ? G.ramp : G.row;
  const inputs = new Map<string, number>();
  for (const s of d.inputs) {
    inputs.set(s.id, y + G.row / 2);
    y += G.row;
  }
  return { w, h: y + G.pad / 2, inputs, outputs, preview };
}

export function nodeBox(node: GNode): Box {
  const g = geometry(node);
  return { x: node.x, y: node.y, w: g.w, h: g.h };
}

/** A socket's center in graph units. A reroute's sockets share its center. */
export function socketPos(
  node: GNode,
  side: "input" | "output",
  id: string,
): P {
  const g = geometry(node);
  const y = node.y + ((side === "input" ? g.inputs : g.outputs).get(id) ?? 0);
  if (node.kind === "reroute") return { x: node.x + g.w / 2, y };
  return { x: side === "input" ? node.x : node.x + g.w, y };
}

/** A wire from an output at `a` to an input at `b`: horizontal tangents,
 *  stretched further when the link runs backwards. */
export function curve(a: P, b: P): [P, P, P, P] {
  const dx = b.x - a.x;
  const reach =
    dx >= 0
      ? Math.max(24, dx * 0.5)
      : Math.max(60, -dx * 0.35 + Math.abs(b.y - a.y) * 0.25);
  return [a, { x: a.x + reach, y: a.y }, { x: b.x - reach, y: b.y }, b];
}

export function bezier([a, b, c, d]: readonly P[], t: number): P {
  const s = 1 - t;
  const k = [s * s * s, 3 * s * s * t, 3 * s * t * t, t * t * t];
  return {
    x: k[0] * a.x + k[1] * b.x + k[2] * c.x + k[3] * d.x,
    y: k[0] * a.y + k[1] * b.y + k[2] * c.y + k[3] * d.y,
  };
}

/** The curve of every link, keyed by `linkKey`. */
export function linkCurves(doc: Doc): Map<string, [P, P, P, P]> {
  const curves = new Map<string, [P, P, P, P]>();
  for (const l of doc.links) {
    const from = doc.nodes[l.from];
    const to = doc.nodes[l.to];
    if (!from || !to) continue;
    curves.set(
      linkKey(l),
      curve(
        socketPos(from, "output", l.output),
        socketPos(to, "input", l.input),
      ),
    );
  }
  return curves;
}

const intersects = (a: Box, b: Box) =>
  a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

export function boxFrom(a: P, b: P): Box {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    w: Math.abs(a.x - b.x),
    h: Math.abs(a.y - b.y),
  };
}

export function nodesInBox(doc: Doc, box: Box): NodeId[] {
  return nodeList(doc)
    .filter((n) => intersects(nodeBox(n), box))
    .map((n) => n.id);
}

/** The topmost node under a point. */
export function nodeAt(doc: Doc, p: P): NodeId | null {
  const hit = nodeList(doc).filter((n) =>
    intersects(nodeBox(n), { ...p, w: 0, h: 0 }),
  );
  return hit.length ? hit[hit.length - 1].id : null;
}

export function bounds(doc: Doc, ids: readonly NodeId[]): Box | null {
  const boxes = ids
    .map((id) => doc.nodes[id])
    .filter(Boolean)
    .map(nodeBox);
  if (!boxes.length) return null;
  const x = Math.min(...boxes.map((b) => b.x));
  const y = Math.min(...boxes.map((b) => b.y));
  const r = Math.max(...boxes.map((b) => b.x + b.w));
  const bottom = Math.max(...boxes.map((b) => b.y + b.h));
  return { x, y, w: r - x, h: bottom - y };
}

export interface SocketRef {
  node: NodeId;
  side: "input" | "output";
  id: string;
}

/** The nearest socket on `side` within `radius` graph units of `p`,
 *  skipping the node a wire is being dragged from. */
export function nearestSocket(
  doc: Doc,
  p: P,
  side: "input" | "output",
  radius: number,
  skip: NodeId,
): SocketRef | null {
  let best: SocketRef | null = null;
  let bestD = radius;
  for (const n of nodeList(doc)) {
    if (n.id === skip) continue;
    const d = def(n.kind);
    for (const s of side === "input" ? d.inputs : d.outputs) {
      const q = socketPos(n, side, s.id);
      const dist = Math.hypot(q.x - p.x, q.y - p.y);
      if (dist < bestD) {
        bestD = dist;
        best = { node: n.id, side, id: s.id };
      }
    }
  }
  return best;
}

/** Where segment `a→b` crosses segment `c→d`, if it does. */
function cross(a: P, b: P, c: P, d: P): P | null {
  const r = { x: b.x - a.x, y: b.y - a.y };
  const s = { x: d.x - c.x, y: d.y - c.y };
  const den = r.x * s.y - r.y * s.x;
  if (Math.abs(den) < 1e-9) return null;
  const t = ((c.x - a.x) * s.y - (c.y - a.y) * s.x) / den;
  const u = ((c.x - a.x) * r.y - (c.y - a.y) * r.x) / den;
  return t >= 0 && t <= 1 && u >= 0 && u <= 1
    ? { x: a.x + r.x * t, y: a.y + r.y * t }
    : null;
}

/** The links a drawn stroke crosses, with the first crossing point of each
 *  (graph units). */
export function crossings(doc: Doc, stroke: readonly P[]): Map<string, P> {
  const hits = new Map<string, P>();
  for (const [key, c] of linkCurves(doc)) {
    const pts = Array.from({ length: 25 }, (_, i) => bezier(c, i / 24));
    outer: for (let i = 1; i < stroke.length; i++) {
      for (let j = 1; j < pts.length; j++) {
        const at = cross(stroke[i - 1], stroke[i], pts[j - 1], pts[j]);
        if (at) {
          hits.set(key, at);
          break outer;
        }
      }
    }
  }
  return hits;
}
