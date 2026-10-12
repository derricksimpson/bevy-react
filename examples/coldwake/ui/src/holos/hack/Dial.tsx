import { C } from "../../theme";
import { arc, polar } from "../suit/arcs";
import { TUMBLERS } from "./useBypass";

/** The dial's size, logical px. */
export const DIAL = 300;
const R = DIAL / 2;
/** The tumbler rings, outermost first. */
const RINGS = [122, 104, 86];

/** Mix two `#rrggbb` colors. */
function mix(a: string, b: string, t: number): string {
  const ch = (c: string, i: number) =>
    parseInt(c.slice(1 + i * 2, 3 + i * 2), 16);
  const out = [0, 1, 2].map((i) =>
    Math.round(ch(a, i) + (ch(b, i) - ch(a, i)) * t)
      .toString(16)
      .padStart(2, "0"),
  );
  return `#${out.join("")}`;
}

/** The rotary lock: a bezel of ticks, three tumbler rings (green once
 *  locked, the next one lit), the pick, and a core that swells as the pick
 *  nears the tumbler's angle. */
export function Dial({
  angle,
  solved,
  near,
  done,
}: {
  angle: number;
  solved: number;
  near: number;
  done: boolean;
}) {
  const pick = mix(C.cyan, C.white, near);
  const box = { positionType: "absolute", width: DIAL, height: DIAL } as const;
  return (
    <node style={{ width: DIAL, height: DIAL }}>
      <svg viewBox={`0 0 ${DIAL} ${DIAL}`} style={box}>
        <circle cx={R} cy={R} r={146} fill="rgba(2, 8, 12, 0.55)" />
        <circle
          cx={R}
          cy={R}
          r={146}
          fill="none"
          stroke={C.cyanDim}
          strokeWidth={1.5}
        />
        {Array.from({ length: 60 }, (_, i) => {
          const major = i % 5 === 0;
          const [x0, y0] = polar(R, R, 142, i * 6);
          const [x1, y1] = polar(R, R, major ? 131 : 137, i * 6);
          return (
            <line
              key={i}
              x1={x0}
              y1={y0}
              x2={x1}
              y2={y1}
              stroke={major ? C.cyan : C.cyanDim}
              strokeWidth={major ? 2 : 1}
            />
          );
        })}
        {RINGS.map((r, i) => {
          const locked = i < solved;
          const current = i === solved && !done;
          return (
            <circle
              key={r}
              cx={R}
              cy={R}
              r={r}
              fill="none"
              stroke={locked ? C.green : current ? C.cyan : C.cyanDeep}
              strokeWidth={locked ? 6 : current ? 3 : 2}
              opacity={locked || current ? 0.95 : 0.8}
            />
          );
        })}
      </svg>
      {/* The pick: a needle turned by the node's transform (no re-raster). */}
      <svg
        viewBox={`0 0 ${DIAL} ${DIAL}`}
        style={{ ...box, transform: { rotate: angle } }}
      >
        {!done && (
          <path
            d={arc(R, R, RINGS[solved], -14, 14)}
            fill="none"
            stroke={pick}
            strokeWidth={9}
            opacity={near * near}
          />
        )}
        <line
          x1={R}
          y1={R - 30}
          x2={R}
          y2={R - 134}
          stroke={pick}
          strokeWidth={2.5 + near * 2}
        />
        <polygon
          points={[R, R - 146, R - 7, R - 132, R + 7, R - 132]}
          fill={pick}
        />
      </svg>
      <svg viewBox={`0 0 ${DIAL} ${DIAL}`} style={box}>
        <circle
          cx={R}
          cy={R}
          r={24 + near * 16}
          fill={done ? C.green : pick}
          opacity={done ? 0.5 : 0.12 + near * near * 0.7}
        />
        <circle
          cx={R}
          cy={R}
          r={22}
          fill="none"
          stroke={done ? C.green : C.cyan}
          strokeWidth={2}
        />
        {Array.from({ length: TUMBLERS }, (_, i) => (
          <circle
            key={i}
            cx={R - 12 + i * 12}
            cy={R}
            r={3.5}
            fill={i < solved ? C.green : C.cyanDim}
          />
        ))}
      </svg>
    </node>
  );
}
