import type { Signal } from "../bevy";
import { type Kind, KINDS, cardHeight, kind } from "./catalog";

/** The patch: React's state. Bevy only hears which sockets are wired
 *  (`patch.set`) and draws the wires itself. */

export type GraphNode = {
  id: string;
  kind: Kind;
  /** Top-left, world px (logical px at zoom 1). */
  x: number;
  y: number;
  params: Record<string, number>;
  /** Stacking order among nodes: the last one pressed is on top. */
  z: number;
  /** Delay before the pop-in, ms. */
  enter: number;
  /** Shrinking away; removed once the animation ends. */
  leaving?: boolean;
};

export type End = { node: string; port: number };
/** Always from an output to an input. */
export type Link = { id: string; from: End; to: End };

export type Graph = {
  nodes: Record<string, GraphNode>;
  links: Link[];
  selected: string[];
  /** Id counter, and the top of the stacking order. */
  seq: number;
  top: number;
};

export type Side = "i" | "o";
export type SocketRef = { node: string; side: Side; port: number };

/** The named node a socket renders — what Bevy finds its position by. */
export const socketName = (s: SocketRef) => `${s.node}:${s.side}${s.port}`;

export function signalOf(g: Graph, s: SocketRef): Signal {
  const d = kind(g.nodes[s.node].kind);
  return (s.side === "o" ? d.outputs : d.inputs)[s.port].signal;
}

/** Whether a wire pulled from `a` may plug into `b`. */
export function canPlug(g: Graph, a: SocketRef, b: SocketRef): boolean {
  return (
    a.side !== b.side &&
    a.node !== b.node &&
    !!g.nodes[b.node] &&
    !g.nodes[b.node].leaving &&
    signalOf(g, a) === signalOf(g, b)
  );
}

export type Box = { x: number; y: number; w: number; h: number };

export const nodeBox = (n: GraphNode): Box => ({
  x: n.x,
  y: n.y,
  w: kind(n.kind).width,
  h: cardHeight(n.kind),
});

export function bounds(nodes: GraphNode[]): Box | null {
  if (!nodes.length) return null;
  const boxes = nodes.map(nodeBox);
  const x = Math.min(...boxes.map((b) => b.x));
  const y = Math.min(...boxes.map((b) => b.y));
  const w = Math.max(...boxes.map((b) => b.x + b.w)) - x;
  const h = Math.max(...boxes.map((b) => b.y + b.h)) - y;
  return { x, y, w, h };
}

export const live = (g: Graph) =>
  Object.values(g.nodes).filter((n) => !n.leaving);

export const EMPTY: Graph = {
  nodes: {},
  links: [],
  selected: [],
  seq: 1,
  top: 1,
};

export type Action =
  | { type: "load"; graph: Graph }
  | { type: "links"; links: Link[] }
  | { type: "add"; kind: Kind; x: number; y: number; select?: boolean }
  | { type: "move"; to: Record<string, { x: number; y: number }> }
  | { type: "select"; ids: string[] }
  | { type: "raise"; ids: string[] }
  | { type: "delete"; ids: string[] }
  | { type: "purge"; id: string }
  | { type: "connect"; a: SocketRef; b: SocketRef }
  | { type: "unplug"; id: string }
  | { type: "param"; id: string; key: string; value: number }
  | { type: "duplicate" };

export function reduce(g: Graph, a: Action): Graph {
  switch (a.type) {
    case "load":
      // Ids stay unique across loads.
      return {
        ...a.graph,
        seq: Math.max(a.graph.seq, g.seq),
        top: Math.max(a.graph.top, g.top),
      };
    case "links":
      return { ...g, links: [...g.links, ...a.links] };
    case "add": {
      const id = `n${g.seq}`;
      const node: GraphNode = {
        id,
        kind: a.kind,
        x: Math.round(a.x),
        y: Math.round(a.y),
        params: { ...KINDS[a.kind].params },
        z: g.top + 1,
        enter: 0,
      };
      return {
        ...g,
        nodes: { ...g.nodes, [id]: node },
        selected: a.select ? [id] : g.selected,
        seq: g.seq + 1,
        top: g.top + 1,
      };
    }
    case "move": {
      const nodes = { ...g.nodes };
      for (const [id, p] of Object.entries(a.to)) {
        if (nodes[id]) nodes[id] = { ...nodes[id], x: p.x, y: p.y };
      }
      return { ...g, nodes };
    }
    case "select":
      return {
        ...g,
        selected: a.ids.filter((id) => g.nodes[id] && !g.nodes[id].leaving),
      };
    case "raise": {
      const nodes = { ...g.nodes };
      let top = g.top;
      for (const id of a.ids) {
        if (nodes[id]) nodes[id] = { ...nodes[id], z: ++top };
      }
      return { ...g, nodes, top };
    }
    case "delete": {
      const gone = new Set(a.ids);
      const nodes = { ...g.nodes };
      for (const id of gone) {
        if (nodes[id]) nodes[id] = { ...nodes[id], leaving: true };
      }
      return {
        ...g,
        nodes,
        links: g.links.filter(
          (l) => !gone.has(l.from.node) && !gone.has(l.to.node),
        ),
        selected: g.selected.filter((id) => !gone.has(id)),
      };
    }
    case "purge": {
      if (!g.nodes[a.id]?.leaving) return g;
      const nodes = { ...g.nodes };
      delete nodes[a.id];
      return { ...g, nodes };
    }
    case "connect": {
      if (!canPlug(g, a.a, a.b)) return g;
      const [out, inp] = a.a.side === "o" ? [a.a, a.b] : [a.b, a.a];
      const from = { node: out.node, port: out.port };
      const to = { node: inp.node, port: inp.port };
      // An input takes one wire: the new one replaces it.
      const links = g.links.filter(
        (l) => !(l.to.node === to.node && l.to.port === to.port),
      );
      links.push({ id: `w${g.seq}`, from, to });
      return { ...g, links, seq: g.seq + 1 };
    }
    case "unplug":
      return { ...g, links: g.links.filter((l) => l.id !== a.id) };
    case "param": {
      const n = g.nodes[a.id];
      if (!n) return g;
      const params = { ...n.params, [a.key]: a.value };
      return { ...g, nodes: { ...g.nodes, [a.id]: { ...n, params } } };
    }
    case "duplicate": {
      const copies: Record<string, string> = {};
      const nodes = { ...g.nodes };
      let seq = g.seq;
      let top = g.top;
      for (const id of g.selected) {
        const n = g.nodes[id];
        const copy = `n${seq++}`;
        copies[id] = copy;
        nodes[copy] = {
          ...n,
          id: copy,
          x: n.x + 32,
          y: n.y + 32,
          z: ++top,
          enter: 0,
        };
      }
      // Wires between the copied nodes come along.
      const links = [...g.links];
      for (const l of g.links) {
        if (copies[l.from.node] && copies[l.to.node]) {
          links.push({
            id: `w${seq++}`,
            from: { node: copies[l.from.node], port: l.from.port },
            to: { node: copies[l.to.node], port: l.to.port },
          });
        }
      }
      return { ...g, nodes, links, selected: Object.values(copies), seq, top };
    }
  }
}

const GAP_X = 64;
const GAP_Y = 32;

/** Lay the patch out left to right by how far each node is from a source,
 *  each column ordered by where its inputs come from. */
export function arrange(g: Graph): Record<string, { x: number; y: number }> {
  const nodes = live(g);
  const depth: Record<string, number> = {};
  const walk = (id: string, seen: Set<string>): number => {
    if (depth[id] !== undefined) return depth[id];
    if (seen.has(id)) return 0;
    seen.add(id);
    const ups = g.links.filter((l) => l.to.node === id).map((l) => l.from.node);
    const d = ups.length ? Math.max(...ups.map((u) => walk(u, seen) + 1)) : 0;
    return (depth[id] = d);
  };
  for (const n of nodes) walk(n.id, new Set());

  const columns: GraphNode[][] = [];
  for (const n of nodes) (columns[depth[n.id]] ??= []).push(n);
  const placed: Record<string, { x: number; y: number }> = {};
  let x = 0;
  for (const column of columns.filter(Boolean)) {
    // Barycenter: under the nodes that feed it, else where it was.
    const key = (n: GraphNode) => {
      const ups = g.links
        .filter((l) => l.to.node === n.id && placed[l.from.node])
        .map((l) => placed[l.from.node].y);
      return ups.length ? ups.reduce((s, v) => s + v, 0) / ups.length : n.y;
    };
    column.sort((a, b) => key(a) - key(b));
    const height = column.reduce(
      (s, n) => s + cardHeight(n.kind) + GAP_Y,
      -GAP_Y,
    );
    let y = -height / 2;
    for (const n of column) {
      placed[n.id] = { x, y };
      y += cardHeight(n.kind) + GAP_Y;
    }
    x += Math.max(...column.map((n) => kind(n.kind).width)) + GAP_X;
  }
  return placed;
}
