import { useRef } from "react";
import { bevy } from "../../bevy";
import { OBJECTIVES } from "../../game/lore";
import { objective } from "../../game/reduce";
import type { Destination } from "../../game/state";
import { useGame } from "../../game/store";
import { C, F, T } from "../../theme";
import { Corners, Rule } from "../shared/kit";

/** The map's legend; the colors match the markers `preview/map.rs` puts on the
 *  deck miniature. */
const PLACES: { id: Destination; name: string; color: string }[] = [
  { id: "store", name: "STORE", color: C.amber },
  { id: "bench", name: "UPGRADE BENCH", color: C.green },
  { id: "save", name: "SAVE STATION", color: C.cyan },
  { id: "elevator", name: "FREIGHT ELEVATOR", color: C.white },
  { id: "bulkhead", name: "BULKHEAD C-D", color: C.red },
];

/** The deck map: a live miniature of Concourse C you turn by dragging and
 *  zoom with the wheel, and what is where. */
export function MapTab({ active }: { active: boolean }) {
  const last = useRef<{ x: number; y: number } | null>(null);
  return (
    <node style={{ flexDirection: "row", gap: 26, flexGrow: 1 }}>
      <node
        style={{
          width: 800,
          flexShrink: 0,
          height: 476,
          border: 1,
          borderColor: "rgba(127, 227, 255, 0.3)",
          backgroundColor: "rgba(2, 10, 16, 0.45)",
        }}
      >
        {active && (
          <portal
            target="map"
            style={{
              width: "100%",
              height: "100%",
              cursor: "grab",
              // A live camera's pixels: re-capture every frame.
              cache: "never",
            }}
            onPointerDown={(e) => {
              last.current = { x: e.clientX, y: e.clientY };
            }}
            onPointerMove={(e) => {
              if (!last.current) return;
              const dx = e.clientX - last.current.x;
              const dy = e.clientY - last.current.y;
              last.current = { x: e.clientX, y: e.clientY };
              if (dx || dy) bevy.map.orbit({ dx, dy });
            }}
            onPointerUp={() => {
              last.current = null;
            }}
            onWheel={(e) =>
              bevy.map.zoom({
                delta: -e.deltaY * (e.deltaMode === "line" ? 1 : 0.025),
              })
            }
          />
        )}
        <Corners size={22} width={2} />
        <text
          style={{
            ...T.small,
            positionType: "absolute",
            left: 14,
            top: 10,
            letterSpacing: 3,
            color: C.cyan,
          }}
        >
          DECK 2 · MAINTENANCE CONCOURSE C
        </text>
        <node
          style={{
            positionType: "absolute",
            left: 14,
            bottom: 10,
            flexDirection: "row",
            gap: 26,
          }}
        >
          <text style={{ ...T.small, letterSpacing: 2 }}>DRAG · TURN</text>
          <text style={{ ...T.small, letterSpacing: 2 }}>WHEEL · ZOOM</text>
        </node>
      </node>
      <Legend />
    </node>
  );
}

function Legend() {
  const fixtures = useGame((g) => g.fixtures);
  const current = useGame(objective);
  const target: Destination = OBJECTIVES[current].to;
  const door = { locked: "LOCKED", unlocked: "UNLOCKED", open: "OPEN" }[
    fixtures.door
  ];
  return (
    <node style={{ flexDirection: "column", gap: 12, flexGrow: 1 }}>
      <text style={{ ...T.small, letterSpacing: 3 }}>LEGEND</text>
      {PLACES.map((p) => (
        <node
          key={p.id}
          style={{ flexDirection: "row", alignItems: "center", gap: 12 }}
        >
          <node
            style={{
              width: 12,
              height: 12,
              borderRadius: 6,
              backgroundColor:
                p.id === "bulkhead" && fixtures.door !== "locked"
                  ? C.green
                  : p.color,
            }}
          />
          <text style={{ ...T.label, fontSize: 18, flexGrow: 1 }}>
            {p.name}
          </text>
          {p.id === target && <Tag text="OBJECTIVE" color={C.amber} />}
          {p.id === fixtures.locator && <Tag text="LOCATOR" color={C.cyan} />}
        </node>
      ))}
      <node style={{ flexDirection: "row", alignItems: "center", gap: 12 }}>
        <svg viewBox="0 0 12 12" style={{ width: 12, height: 12 }}>
          <polygon points={[6, 0, 12, 12, 0, 12]} fill={C.white} />
        </svg>
        <text style={{ ...T.label, fontSize: 18 }}>YOU</text>
      </node>
      <Rule />
      <Line
        label="BULKHEAD C-D"
        value={door}
        color={fixtures.door === "locked" ? C.red : C.green}
      />
      <Line
        label="ELEVATOR"
        value={`DECK ${fixtures.elevator.floor}`}
        color={C.white}
      />
      <Line
        label="OBJECTIVE"
        value={OBJECTIVES[current].title}
        color={C.amber}
      />
    </node>
  );
}

function Tag({ text, color }: { text: string; color: string }) {
  return (
    <node
      style={{
        border: 1,
        borderColor: color,
        padding: { horizontal: 6, vertical: 1 },
      }}
    >
      <text
        style={{
          fontFamily: F.semibold,
          fontSize: 12,
          letterSpacing: 2,
          color,
        }}
      >
        {text}
      </text>
    </node>
  );
}

function Line({
  label,
  value,
  color,
}: {
  label: string;
  value: string;
  color: string;
}) {
  return (
    <node style={{ flexDirection: "column", gap: 1 }}>
      <text style={{ ...T.small, letterSpacing: 2 }}>{label}</text>
      <text
        style={{
          fontFamily: F.semibold,
          fontSize: 17,
          letterSpacing: 1.5,
          color,
        }}
      >
        {value}
      </text>
    </node>
  );
}
