import type { Value } from "../bevy";
import { geometry } from "../graph/layout";
import { type Doc, type NodeId, emptyDoc } from "../graph/model";

/** `[key, kind, values?]` — a node of a preset. */
export type NodeSpec = [string, string, Record<string, Value>?];
/** `"from.output > to.input"`. */
export type LinkSpec = string;

const COLUMN = 240;
const GAP = 34;

/** A preset document: nodes in columns, left to right, each column stacked
 *  from the top (heights from the cards' own geometry, so nothing overlaps);
 *  the last column is centered on the tallest. */
export function build(columns: NodeSpec[][], links: LinkSpec[]): Doc {
  const doc = emptyDoc();
  const ids = new Map<string, NodeId>();
  const heights: number[] = [];
  columns.forEach((column, c) => {
    let y = 0;
    for (const [key, kind, values = {}] of column) {
      const id = doc.nextId++;
      ids.set(key, id);
      const node = { id, kind, x: c * COLUMN, y, values };
      doc.nodes[id] = node;
      y += geometry(node).h + GAP;
    }
    heights.push(y - GAP);
  });
  const tallest = Math.max(...heights);
  const last = columns.length - 1;
  for (const [key] of columns[last])
    doc.nodes[ids.get(key)!].y += (tallest - heights[last]) / 2;

  for (const spec of links) {
    const [from, to] = spec.split(">").map((s) => s.trim());
    const [a, output] = from.split(".");
    const [b, input] = to.split(".");
    const [fromId, toId] = [ids.get(a), ids.get(b)];
    if (fromId === undefined || toId === undefined)
      throw new Error(`bad preset link ${spec}`);
    doc.links.push({ from: fromId, output, to: toId, input });
  }
  return doc;
}

/** A ramp's stops from `[pos, [r, g, b]]` pairs. */
export const stops = (...s: [number, [number, number, number]][]) =>
  s.map(([pos, color]) => ({ pos, color }));
