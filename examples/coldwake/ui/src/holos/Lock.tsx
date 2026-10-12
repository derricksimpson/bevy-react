import { useGame } from "../game/store";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { arc } from "./suit/arcs";
import { useSpin } from "./suit/loops";

/** The bulkhead's lock (200 × 200): turning rings around a padlock, red
 *  while sealed, green once the bypass opened it. */
export function Lock() {
  const door = useGame((g) => g.fixtures.door);
  const locked = door === "locked";
  const color = locked ? C.red : C.green;
  const dim = locked ? C.redDim : C.greenDim;
  const outer = useSpin(16);
  const inner = useSpin(-9);
  const ring = { positionType: "absolute", width: 200, height: 200 } as const;

  return (
    <Holo
      name="lock"
      seed={13}
      style={{ alignItems: "center", justifyContent: "center" }}
    >
      <svg
        viewBox="0 0 200 200"
        style={{ ...ring, transform: { rotate: { animated: outer } } }}
      >
        {Array.from({ length: 8 }, (_, i) => (
          <path
            key={i}
            d={arc(100, 100, 92, i * 45 + 4, i * 45 + 37)}
            fill="none"
            stroke={color}
            strokeWidth={i % 2 ? 2 : 5}
            opacity={i % 2 ? 0.5 : 0.9}
          />
        ))}
      </svg>
      <svg
        viewBox="0 0 200 200"
        style={{ ...ring, transform: { rotate: { animated: inner } } }}
      >
        <circle
          cx={100}
          cy={100}
          r={78}
          fill="none"
          stroke={dim}
          strokeWidth={1.5}
        />
        {[0, 120, 240].map((a) => (
          <path
            key={a}
            d={arc(100, 100, 78, a - 8, a + 8)}
            fill="none"
            stroke={color}
            strokeWidth={6}
          />
        ))}
      </svg>
      <svg viewBox="0 0 200 200" style={ring}>
        <circle cx={100} cy={100} r={58} fill="rgba(2, 8, 12, 0.6)" />
        {/* The padlock: its shackle lifts open once unlocked. */}
        <path
          d={
            locked
              ? "M84 92 L84 78 A16 16 0 0 1 116 78 L116 92"
              : "M84 80 L84 66 A16 16 0 0 1 116 66 L116 74"
          }
          fill="none"
          stroke={color}
          strokeWidth={6}
        />
        <rect x={74} y={92} width={52} height={38} rx={4} fill={color} />
        <rect x={97} y={102} width={6} height={14} fill="#05080c" />
      </svg>
      <node style={{ positionType: "absolute", bottom: 48 }}>
        <text
          style={{
            fontFamily: F.bold,
            fontSize: 17,
            letterSpacing: 3,
            color,
            lineBreak: "noWrap",
          }}
        >
          {locked ? "LOCKED" : "UNLOCKED"}
        </text>
      </node>
    </Holo>
  );
}
