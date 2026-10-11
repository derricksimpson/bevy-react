import type { WeaponId } from "./items";

/** What the bench upgrades: the three weapons and the suit (the RIG). */
export type BenchTarget = WeaponId | "rig";
export const BENCH_TARGETS: BenchTarget[] = ["cutter", "pulse", "rail", "rig"];

/** A circuit node's effect. `start` is the circuit's input (always
 *  powered); `empty` is a junction that only carries power on. */
export type NodeKind =
  | "start"
  | "damage"
  | "capacity"
  | "reload"
  | "special"
  | "empty";

export type CircuitNode = {
  id: string;
  /** Grid cell (6 × 3). */
  x: number;
  y: number;
  kind: NodeKind;
  /** Neighbours a powered node passes power to. */
  links: string[];
};

/** Every circuit has the same board; the targets differ in which node does
 *  what. A node can be powered (costs one power cell) when a neighbour is. */
const BOARD: [string, number, number, string[]][] = [
  ["start", 0, 1, ["a"]],
  ["a", 1, 1, ["start", "b", "a2"]],
  ["a2", 1, 0, ["a"]],
  ["b", 2, 1, ["a", "c", "b2"]],
  ["b2", 2, 2, ["b"]],
  ["c", 3, 1, ["b", "d", "c2"]],
  ["c2", 3, 0, ["c"]],
  ["d", 4, 1, ["c", "e", "d2"]],
  ["d2", 4, 2, ["d"]],
  ["e", 5, 1, ["d"]],
];

function circuit(kinds: Record<string, NodeKind>): CircuitNode[] {
  return BOARD.map(([id, x, y, links]) => ({
    id,
    x,
    y,
    links,
    kind: id === "start" ? "start" : (kinds[id] ?? "empty"),
  }));
}

export const CIRCUITS: Record<BenchTarget, CircuitNode[]> = {
  cutter: circuit({
    a: "damage",
    a2: "reload",
    b: "capacity",
    b2: "capacity",
    c2: "reload",
    d: "damage",
    d2: "capacity",
    e: "special",
  }),
  pulse: circuit({
    a: "capacity",
    a2: "damage",
    b: "reload",
    b2: "capacity",
    c: "damage",
    d2: "reload",
    d: "capacity",
    e: "special",
  }),
  rail: circuit({
    a: "damage",
    a2: "capacity",
    b2: "reload",
    c: "damage",
    c2: "capacity",
    d: "reload",
    e: "special",
  }),
  rig: circuit({
    a: "damage",
    a2: "capacity",
    b: "reload",
    c: "capacity",
    c2: "damage",
    d2: "reload",
    d: "damage",
    e: "special",
  }),
};

/** How each node kind reads on each target (the RIG's "damage" is armor,
 *  its "capacity" air, its "reload" stasis). */
export const STAT_NAMES: Record<BenchTarget, Record<NodeKind, string>> = {
  cutter: names("DAMAGE", "CAPACITY", "RELOAD", "SUPERHEATED ARC"),
  pulse: names("DAMAGE", "CAPACITY", "RELOAD", "DRIVER MINE"),
  rail: names("DAMAGE", "CAPACITY", "RELOAD", "PINNING SPIKE"),
  rig: names("ARMOR", "AIR", "STASIS", "AUTO-STASIS"),
};

function names(
  damage: string,
  capacity: string,
  reload: string,
  special: string,
): Record<NodeKind, string> {
  return {
    start: "INPUT",
    empty: "JUNCTION",
    damage,
    capacity,
    reload,
    special,
  };
}

/** Base values and per-node steps: weapons (damage, clip, reload s) and
 *  the RIG (armor %, air s, stasis s). */
const STATS: Record<
  BenchTarget,
  {
    damage: [number, number];
    capacity: [number, number];
    reload: [number, number];
  }
> = {
  cutter: { damage: [10, 3], capacity: [10, 2], reload: [1.6, -0.2] },
  pulse: { damage: [6, 1], capacity: [25, 5], reload: [2.0, -0.25] },
  rail: { damage: [30, 8], capacity: [4, 1], reload: [2.4, -0.3] },
  rig: { damage: [0, 10], capacity: [60, 15], reload: [4, 1] },
};

/** The target's stats with `installed` nodes powered. */
export function stats(target: BenchTarget, installed: string[]) {
  const nodes = CIRCUITS[target].filter((n) => installed.includes(n.id));
  const count = (kind: NodeKind) => nodes.filter((n) => n.kind === kind).length;
  const s = STATS[target];
  const at = ([base, step]: [number, number], kind: NodeKind) =>
    Math.round((base + step * count(kind)) * 100) / 100;
  return {
    damage: at(s.damage, "damage"),
    capacity: at(s.capacity, "capacity"),
    reload: at(s.reload, "reload"),
    special: count("special") > 0,
  };
}

/** Whether `node` can be powered next: not yet, and next to a powered one. */
export function canInstall(
  target: BenchTarget,
  installed: string[],
  node: string,
): boolean {
  if (node === "start" || installed.includes(node)) return false;
  const n = CIRCUITS[target].find((c) => c.id === node);
  return !!n && n.links.some((l) => l === "start" || installed.includes(l));
}
