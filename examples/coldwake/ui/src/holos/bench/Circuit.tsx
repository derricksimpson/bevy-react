import { useEffect } from "react";
import { useSharedValue, withRepeat, withTiming } from "bevy-react";
import {
  CIRCUITS,
  STAT_NAMES,
  canInstall,
  type BenchTarget,
  type CircuitNode,
  type NodeKind,
} from "../../game/bench";
import { C, F } from "../../theme";
import { useLive } from "../Holo";

/** The board's size and a grid cell's (6 × 3 cells). */
export const BOARD = { width: 780, height: 450 };
const CELL = { x: BOARD.width / 6, y: BOARD.height / 3 };
const R = 30;

/** Each node kind's light. */
export const KIND_COLOR: Record<NodeKind, string> = {
  start: C.white,
  damage: C.red,
  capacity: C.cyan,
  reload: C.amber,
  special: C.white,
  empty: C.cyanDim,
};

const SHORT: Record<NodeKind, string> = {
  start: "IN",
  damage: "DMG",
  capacity: "CAP",
  reload: "RLD",
  special: "SPC",
  empty: "—",
};

/** The RIG's nodes are armor, air and stasis. */
const RIG_SHORT: Record<NodeKind, string> = {
  ...SHORT,
  damage: "ARM",
  capacity: "AIR",
  reload: "STS",
};

function center(n: CircuitNode) {
  return { x: CELL.x * (n.x + 0.5), y: CELL.y * (n.y + 0.5) };
}

/** A circuit board: traces between the nodes, each node a ring in its
 *  kind's color — powered ones filled with light, the ones power can reach
 *  next breathing, the rest dark. The svg only draws; a transparent button
 *  over each node takes the clicks and the hover. */
export function Circuit({
  target,
  installed,
  hover,
  onHover,
  onInstall,
}: {
  target: BenchTarget;
  installed: string[];
  hover: string | null;
  onHover: (node: string | null) => void;
  onInstall: (node: string) => void;
}) {
  const nodes = CIRCUITS[target];
  const powered = (id: string) => id === "start" || installed.includes(id);
  // Only while he is at the bench: each frame of the pulse re-rasters the
  // board.
  const live = useLive("bench");
  const pulse = useSharedValue(0.6);
  useEffect(() => {
    pulse.value = live
      ? withRepeat(withTiming(1, { duration: 700, easing: "easeInOut" }), {
          reverse: true,
        })
      : withTiming(0.6, { duration: 200 });
  }, [pulse, live]);

  // Each link once (the board lists them both ways).
  const links = nodes.flatMap((n) =>
    n.links
      .filter((l) => n.id < l)
      .map((l) => [n, nodes.find((m) => m.id === l)!] as const),
  );

  return (
    <node style={{ width: BOARD.width, height: BOARD.height }}>
      <svg
        viewBox={`0 0 ${BOARD.width} ${BOARD.height}`}
        style={{ width: BOARD.width, height: BOARD.height }}
      >
        {/* The grid's faint dots: an etched board. */}
        {Array.from({ length: 7 * 4 }, (_, i) => (
          <circle
            key={`dot${i}`}
            cx={(i % 7) * CELL.x}
            cy={Math.floor(i / 7) * CELL.y}
            r={1.5}
            fill="rgba(127, 227, 255, 0.25)"
          />
        ))}
        {links.map(([a, b]) => {
          const p = center(a);
          const q = center(b);
          const live = powered(a.id) && powered(b.id);
          return (
            <line
              key={`${a.id}-${b.id}`}
              x1={p.x}
              y1={p.y}
              x2={q.x}
              y2={q.y}
              stroke={live ? C.cyan : "rgba(127, 227, 255, 0.22)"}
              strokeWidth={live ? 4 : 2}
            />
          );
        })}
        {nodes.map((n) => {
          const { x, y } = center(n);
          const on = powered(n.id);
          const next = canInstall(target, installed, n.id);
          const color = KIND_COLOR[n.kind];
          return (
            <g key={n.id}>
              {next && (
                <circle
                  cx={x}
                  cy={y}
                  r={R + 7}
                  fill="none"
                  stroke={color}
                  strokeWidth={2}
                  opacity={{ animated: pulse }}
                />
              )}
              {n.kind === "start" ? (
                <rect
                  x={x - R}
                  y={y - R}
                  width={R * 2}
                  height={R * 2}
                  fill="rgba(240, 253, 255, 0.18)"
                  stroke={C.white}
                  strokeWidth={3}
                />
              ) : (
                <circle
                  cx={x}
                  cy={y}
                  r={R}
                  fill={on ? color : "rgba(2, 10, 16, 0.7)"}
                  opacity={on ? 0.9 : 1}
                  stroke={on || next ? color : "rgba(127, 227, 255, 0.3)"}
                  strokeWidth={hover === n.id ? 4 : 2.5}
                />
              )}
              {on && n.kind !== "start" && (
                <circle cx={x} cy={y} r={R - 9} fill="rgba(2, 10, 16, 0.55)" />
              )}
            </g>
          );
        })}
      </svg>
      {nodes.map((n) => {
        const { x, y } = center(n);
        const on = powered(n.id);
        const next = canInstall(target, installed, n.id);
        return (
          <button
            key={n.id}
            onClick={() => next && onInstall(n.id)}
            onPointerEnter={() => onHover(n.id)}
            onPointerLeave={() => onHover(null)}
            style={{
              positionType: "absolute",
              left: x - R - 4,
              top: y - R - 4,
              width: (R + 4) * 2,
              height: (R + 4) * 2,
              alignItems: "center",
              justifyContent: "center",
              borderRadius: R + 4,
              cursor: next ? "pointer" : "default",
            }}
            hoverStyle={{ backgroundColor: "rgba(127, 227, 255, 0.1)" }}
          >
            <text
              style={{
                fontFamily: F.semibold,
                fontSize: 13,
                letterSpacing: 1,
                color: on
                  ? C.white
                  : next
                    ? KIND_COLOR[n.kind]
                    : "rgba(127, 227, 255, 0.4)",
              }}
            >
              {(target === "rig" ? RIG_SHORT : SHORT)[n.kind]}
            </text>
          </button>
        );
      })}
    </node>
  );
}

/** The kinds this target's board uses, for the legend. */
export function legend(
  target: BenchTarget,
): { kind: NodeKind; name: string }[] {
  const kinds: NodeKind[] = [
    "damage",
    "capacity",
    "reload",
    "special",
    "empty",
  ];
  return kinds.map((kind) => ({ kind, name: STAT_NAMES[target][kind] }));
}
