import { useState } from "react";
import { install } from "../game/act";
import {
  BENCH_TARGETS,
  CIRCUITS,
  STAT_NAMES,
  canInstall,
  stats,
  type BenchTarget,
} from "../game/bench";
import { ITEMS, countItem } from "../game/items";
import { useGame } from "../game/store";
import { useDebug } from "../hooks";
import { cue } from "../sound";
import { C, F, T } from "../theme";
import { Circuit, KIND_COLOR, legend } from "./bench/Circuit";
import { Holo } from "./Holo";
import { Corners, Rule, Shell, Tabs } from "./shared/kit";

const TABS = BENCH_TARGETS.map((id) => ({
  id,
  label: id === "rig" ? "RIG" : ITEMS[id].name.toUpperCase(),
}));

/** The upgrade bench (beat 5): pick a weapon or the suit, power circuit
 *  nodes with power cells (one each, next to a powered node), watch the
 *  stats move. Hovering a node previews what it would do. */
export function Bench() {
  const owned = useGame((g) => g.arms.owned);
  const bench = useGame((g) => g.bench);
  const cells = useGame((g) => countItem(g.pack, "power-cell"));
  const [target, setTarget] = useState<BenchTarget>("cutter");
  const [hover, setHover] = useState<string | null>(null);
  const locked = BENCH_TARGETS.filter((t) => t !== "rig" && !owned.includes(t));

  const pick = (next: BenchTarget) => {
    if (next === target) return;
    setTarget(next);
    setHover(null);
    cue("tab");
  };
  useDebug("bench", (id) => {
    const next = BENCH_TARGETS.find((t) => t === id);
    if (next && !locked.includes(next)) setTarget(next);
  });
  useDebug("install", (node) => install(target, node));
  useDebug("hover", (node) => setHover(node || null));

  return (
    <Holo name="bench" page={target} seed={7}>
      <Shell>
        <node style={{ flexDirection: "row", alignItems: "center", gap: 18 }}>
          <node style={{ flexDirection: "column", gap: 3 }}>
            <text style={{ ...T.header, fontSize: 28, letterSpacing: 6 }}>
              UPGRADE BENCH
            </text>
            <text style={{ ...T.small, letterSpacing: 2 }}>
              HALDEN TOOLWORKS · CIRCUIT BENCH C-2
            </text>
          </node>
          <node style={{ flexGrow: 1 }} />
          <node style={{ flexDirection: "column", alignItems: "end", gap: 1 }}>
            <text style={{ ...T.small, letterSpacing: 3 }}>POWER CELLS</text>
            <text
              style={{
                ...T.number,
                fontSize: 42,
                color: cells > 0 ? C.green : C.red,
              }}
            >
              {String(cells)}
            </text>
          </node>
        </node>
        <node style={{ margin: { top: 10 } }}>
          <Tabs tabs={TABS} active={target} onPick={pick} disabled={locked} />
        </node>
        <Rule />
        <node
          style={{
            flexDirection: "row",
            gap: 26,
            flexGrow: 1,
            margin: { top: 18 },
          }}
        >
          <node
            style={{
              padding: 4,
              flexShrink: 0,
              border: 1,
              borderColor: "rgba(127, 227, 255, 0.25)",
              backgroundColor: "rgba(2, 12, 20, 0.5)",
            }}
          >
            <Corners size={22} width={2} />
            <Circuit
              target={target}
              installed={bench[target]}
              hover={hover}
              onHover={setHover}
              onInstall={(node) => install(target, node)}
            />
          </node>
          <Stats
            target={target}
            installed={bench[target]}
            hover={hover}
            cells={cells}
          />
        </node>
        <node style={{ flexDirection: "row", gap: 24, margin: { top: 12 } }}>
          {legend(target).map(({ kind, name }) => (
            <node
              key={kind}
              style={{ flexDirection: "row", alignItems: "center", gap: 8 }}
            >
              <node
                style={{
                  width: 12,
                  height: 12,
                  borderRadius: 6,
                  border: 2,
                  borderColor: KIND_COLOR[kind],
                }}
              />
              <text style={{ ...T.small, letterSpacing: 2 }}>{name}</text>
            </node>
          ))}
        </node>
      </Shell>
    </Holo>
  );
}

/** How a stat reads: the RIG's armor in %, times in seconds. */
function format(
  target: BenchTarget,
  stat: "damage" | "capacity" | "reload",
  v: number,
) {
  if (target === "rig") return stat === "damage" ? `${v}%` : `${v} S`;
  return stat === "reload" ? `${v.toFixed(2)} S` : String(v);
}

/** The target's stats now, and — over a node power can reach — what it
 *  would make them. */
function Stats({
  target,
  installed,
  hover,
  cells,
}: {
  target: BenchTarget;
  installed: string[];
  hover: string | null;
  cells: number;
}) {
  const now = stats(target, installed);
  const node = CIRCUITS[target].find((n) => n.id === hover);
  const reachable = !!hover && canInstall(target, installed, hover);
  const then = reachable ? stats(target, [...installed, hover]) : now;
  const names = STAT_NAMES[target];
  const rows = (["damage", "capacity", "reload"] as const).map((stat) => ({
    stat,
    name: names[stat],
    now: now[stat],
    then: then[stat],
  }));
  const state = !node
    ? null
    : node.kind === "start" || installed.includes(node.id)
      ? { text: "POWERED", color: C.green }
      : reachable
        ? { text: "1 POWER CELL TO POWER", color: cells > 0 ? C.amber : C.red }
        : { text: "NO POWER REACHES IT", color: C.cyanDim };
  return (
    <node style={{ flexDirection: "column", gap: 16, flexGrow: 1 }}>
      <text style={{ ...T.small, letterSpacing: 3 }}>
        {target === "rig" ? "RIG MK-II" : ITEMS[target].name.toUpperCase()}
      </text>
      {rows.map((r) => (
        <node key={r.stat} style={{ flexDirection: "column", gap: 4 }}>
          <text style={{ ...T.small, letterSpacing: 2 }}>{r.name}</text>
          <node style={{ flexDirection: "row", alignItems: "center", gap: 14 }}>
            <text style={{ ...T.number, fontSize: 30 }}>
              {format(target, r.stat, r.now)}
            </text>
            {r.then !== r.now && (
              <text style={{ ...T.number, fontSize: 22, color: C.green }}>
                {`› ${format(target, r.stat, r.then)}`}
              </text>
            )}
          </node>
        </node>
      ))}
      <node style={{ flexDirection: "column", gap: 4 }}>
        <text style={{ ...T.small, letterSpacing: 2 }}>{names.special}</text>
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 20,
            letterSpacing: 2,
            color: then.special ? C.green : C.cyanDeep,
          }}
        >
          {now.special ? "ACTIVE" : then.special ? "› ACTIVE" : "LOCKED"}
        </text>
      </node>
      <node style={{ flexGrow: 1 }} />
      <Rule />
      {node && state ? (
        <node style={{ flexDirection: "column", gap: 3 }}>
          <text style={{ ...T.label, color: KIND_COLOR[node.kind] }}>
            {names[node.kind]}
          </text>
          <text style={{ ...T.small, letterSpacing: 2, color: state.color }}>
            {state.text}
          </text>
        </node>
      ) : (
        <text style={{ ...T.small, letterSpacing: 2 }}>
          POWER A NODE NEXT TO A POWERED ONE · 1 CELL EACH
        </text>
      )}
    </node>
  );
}
