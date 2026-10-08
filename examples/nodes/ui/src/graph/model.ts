import type { GraphLink, SetGraph, SocketType, Value } from "../bevy";
import { def } from "../catalog";

export type NodeId = number;

/** One node of the document. Positions are graph units (the node's top
 *  left at zoom 1). */
export interface GNode {
  id: NodeId;
  kind: string;
  x: number;
  y: number;
  /** Parameters and unlinked inputs' values; missing = the catalog default. */
  values: Record<string, Value>;
  mute?: boolean;
  collapsed?: boolean;
  /** A custom title (rename). */
  label?: string;
  /** The live thumbnail is folded away. */
  hidePreview?: boolean;
}

export type GLink = GraphLink;

/** The document: everything undo restores. */
export interface Doc {
  nodes: Record<NodeId, GNode>;
  links: GLink[];
  nextId: NodeId;
}

export const emptyDoc = (): Doc => ({ nodes: {}, links: [], nextId: 1 });

export const nodeList = (doc: Doc): GNode[] => Object.values(doc.nodes);

export const linkKey = (l: { to: NodeId; input: string }) =>
  `${l.to}:${l.input}`;

export function linkInto(
  doc: Doc,
  to: NodeId,
  input: string,
): GLink | undefined {
  return doc.links.find((l) => l.to === to && l.input === input);
}

export function addNode(
  doc: Doc,
  kind: string,
  x: number,
  y: number,
  values: Record<string, Value> = {},
): [Doc, NodeId] {
  const id = doc.nextId;
  const node: GNode = { id, kind, x, y, values };
  return [{ ...doc, nodes: { ...doc.nodes, [id]: node }, nextId: id + 1 }, id];
}

export function updateNodes(
  doc: Doc,
  ids: readonly NodeId[],
  patch: (n: GNode) => Partial<GNode>,
): Doc {
  const nodes = { ...doc.nodes };
  for (const id of ids) {
    const n = nodes[id];
    if (n) nodes[id] = { ...n, ...patch(n) };
  }
  return { ...doc, nodes };
}

export function setValue(doc: Doc, id: NodeId, key: string, value: Value): Doc {
  return updateNodes(doc, [id], (n) => ({
    values: { ...n.values, [key]: value },
  }));
}

export function moveNodes(
  doc: Doc,
  positions: ReadonlyMap<NodeId, { x: number; y: number }>,
): Doc {
  return updateNodes(doc, [...positions.keys()], (n) => positions.get(n.id)!);
}

export function removeNodes(doc: Doc, ids: readonly NodeId[]): Doc {
  const gone = new Set(ids);
  const nodes = { ...doc.nodes };
  for (const id of ids) delete nodes[id];
  const links = doc.links.filter((l) => !gone.has(l.from) && !gone.has(l.to));
  return { ...doc, nodes, links };
}

/** Whether `to` feeds `from` already (so `from → to` would close a loop). */
export function wouldCycle(doc: Doc, from: NodeId, to: NodeId): boolean {
  if (from === to) return true;
  const seen = new Set<NodeId>();
  const stack = [from];
  while (stack.length) {
    const id = stack.pop()!;
    if (id === to) return true;
    if (seen.has(id)) continue;
    seen.add(id);
    for (const l of doc.links) if (l.to === id) stack.push(l.from);
  }
  return false;
}

/** Link an output to an input, replacing whatever fed that input. Refuses
 *  (returns the doc unchanged) a link that would close a loop. */
export function connect(doc: Doc, link: GLink): Doc {
  if (wouldCycle(doc, link.from, link.to)) return doc;
  const links = doc.links.filter((l) => linkKey(l) !== linkKey(link));
  return { ...doc, links: [...links, link] };
}

export function disconnect(doc: Doc, keys: readonly string[]): Doc {
  const gone = new Set(keys);
  return { ...doc, links: doc.links.filter((l) => !gone.has(linkKey(l))) };
}

/** The type flowing out of an output; a reroute takes its source's. */
export function outputType(doc: Doc, id: NodeId, output: string): SocketType {
  const node = doc.nodes[id];
  if (!node) return "any";
  if (node.kind === "reroute") {
    const src = linkInto(doc, id, "in");
    return src ? outputType(doc, src.from, src.output) : "any";
  }
  return def(node.kind).outputs.find((s) => s.id === output)?.type ?? "any";
}

export function inputType(doc: Doc, id: NodeId, input: string): SocketType {
  const node = doc.nodes[id];
  if (!node) return "any";
  if (node.kind === "reroute") return outputType(doc, id, "out");
  return def(node.kind).inputs.find((s) => s.id === input)?.type ?? "any";
}

/** What Bevy evaluates: everything but the layout. */
export function evalGraph(doc: Doc, resolution: number): SetGraph {
  return {
    resolution,
    nodes: nodeList(doc).map((n) => ({
      id: n.id,
      kind: n.kind,
      values: n.values,
      mute: !!n.mute,
    })),
    links: doc.links,
  };
}

/** A copied piece of graph: nodes and the links among them. */
export interface Clip {
  nodes: GNode[];
  links: GLink[];
}

export function copy(doc: Doc, ids: readonly NodeId[]): Clip {
  const set = new Set(ids);
  return {
    nodes: ids.map((id) => doc.nodes[id]).filter(Boolean),
    links: doc.links.filter((l) => set.has(l.from) && set.has(l.to)),
  };
}

/** Insert a clip with fresh ids, its top-left corner at `at`. */
export function paste(
  doc: Doc,
  clip: Clip,
  at: { x: number; y: number },
): [Doc, NodeId[]] {
  if (!clip.nodes.length) return [doc, []];
  const minX = Math.min(...clip.nodes.map((n) => n.x));
  const minY = Math.min(...clip.nodes.map((n) => n.y));
  const ids = new Map<NodeId, NodeId>();
  const nodes = { ...doc.nodes };
  let next = doc.nextId;
  for (const n of clip.nodes) {
    const id = next++;
    ids.set(n.id, id);
    nodes[id] = { ...n, id, x: n.x - minX + at.x, y: n.y - minY + at.y };
  }
  const links = clip.links.map((l) => ({
    ...l,
    from: ids.get(l.from)!,
    to: ids.get(l.to)!,
  }));
  return [
    { nodes, links: [...doc.links, ...links], nextId: next },
    [...ids.values()],
  ];
}
