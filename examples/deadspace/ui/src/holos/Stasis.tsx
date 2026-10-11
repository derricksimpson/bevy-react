import { useGame } from "../game/store";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { arc, polar } from "./suit/arcs";

/** Stasis light: bluer than the rest of the suit's cyan. */
const BLUE = "#6cc8ff";
/** The gauge: a 270° arc open at the bottom, in four charges. */
const START = -135;
const GAP = 6;
const SPAN = (270 - 3 * GAP) / 4;

/** The stasis module's charge on the right shoulder blade (160 × 160): four
 *  quarter segments, one spent per use. */
export function Stasis() {
  return (
    <Holo name="stasis" seed={5}>
      <StasisGauge />
    </Holo>
  );
}

function StasisGauge() {
  const stasis = useGame((g) => g.suit.stasis);
  const level = stasis * 4;

  return (
    <node
      style={{
        width: "100%",
        height: "100%",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <svg
        viewBox="0 0 160 160"
        style={{ positionType: "absolute", width: 160, height: 160 }}
      >
        <circle
          cx={80}
          cy={80}
          r={74}
          fill="none"
          stroke={C.cyanDim}
          strokeWidth={1}
          opacity={0.5}
        />
        {Array.from({ length: 12 }, (_, i) => {
          const [x0, y0] = polar(80, 80, 74, i * 30);
          const [x1, y1] = polar(80, 80, 68, i * 30);
          return (
            <line
              key={i}
              x1={x0}
              y1={y0}
              x2={x1}
              y2={y1}
              stroke={C.cyanDim}
              strokeWidth={1.4}
            />
          );
        })}
        {[0, 1, 2, 3].map((k) => {
          const from = START + k * (SPAN + GAP);
          const lit = Math.max(0, Math.min(1, level - k));
          return (
            <g key={k}>
              <path
                d={arc(80, 80, 56, from, from + SPAN)}
                fill="none"
                stroke={BLUE}
                strokeWidth={13}
                opacity={0.18}
              />
              {lit > 0 && (
                <path
                  d={arc(80, 80, 56, from, from + SPAN * lit)}
                  fill="none"
                  stroke={BLUE}
                  strokeWidth={13}
                  opacity={lit < 1 ? 0.55 : 0.95}
                />
              )}
              {lit >= 1 && (
                <path
                  d={arc(80, 80, 50, from + 2, from + SPAN - 2)}
                  fill="none"
                  stroke={C.white}
                  strokeWidth={1.4}
                />
              )}
            </g>
          );
        })}
      </svg>
      <text
        style={{
          fontFamily: F.display,
          fontSize: 30,
          color: stasis >= 0.25 ? C.white : C.red,
          lineBreak: "noWrap",
        }}
      >
        {Math.round(stasis * 100)}
      </text>
      <text
        style={{
          fontFamily: F.semibold,
          fontSize: 13,
          letterSpacing: 2.5,
          color: BLUE,
          lineBreak: "noWrap",
        }}
      >
        STASIS
      </text>
    </node>
  );
}
