import { ITEMS } from "../game/items";
import { clipSize, reserve } from "../game/reduce";
import { useGame } from "../game/store";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { useBlink } from "./suit/loops";

/** Clips up to this size show one pip per round; bigger ones a bar. */
const MAX_PIPS = 12;

/** The ammo count projected over the gun while aiming (220 × 130). */
export function Ammo() {
  return (
    <Holo name="ammo" seed={11}>
      <AmmoCount />
    </Holo>
  );
}

function AmmoCount() {
  const weapon = useGame((g) => g.arms.weapon);
  const clip = useGame((g) => g.arms.clip[g.arms.weapon]);
  const size = useGame((g) => clipSize(g, g.arms.weapon));
  const spare = useGame((g) => reserve(g, g.arms.weapon));
  const empty = clip === 0;
  const blink = useBlink(empty, 300, 0.15);

  return (
    <node
      style={{
        width: "100%",
        height: "100%",
        padding: { horizontal: 12, vertical: 8 },
        flexDirection: "column",
        justifyContent: "spaceBetween",
      }}
    >
      <node
        style={{
          flexDirection: "row",
          justifyContent: "spaceBetween",
          alignItems: "center",
        }}
      >
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 13,
            letterSpacing: 2,
            color: C.cyan,
            lineBreak: "noWrap",
          }}
        >
          {ITEMS[weapon].name.toUpperCase()}
        </text>
        <TriDot />
      </node>
      <node style={{ flexDirection: "row", alignItems: "flexEnd", gap: 8 }}>
        <text
          style={{
            fontFamily: F.display,
            fontSize: 50,
            color: empty ? C.red : C.white,
            lineBreak: "noWrap",
          }}
        >
          {String(clip).padStart(2, "0")}
        </text>
        <node style={{ flexDirection: "column", padding: { bottom: 8 } }}>
          {empty ? (
            <text
              style={{
                fontFamily: F.bold,
                fontSize: 15,
                letterSpacing: 2,
                color: C.red,
                opacity: { animated: blink },
                lineBreak: "noWrap",
              }}
            >
              RELOAD
            </text>
          ) : null}
          <text
            style={{
              fontFamily: F.display,
              fontSize: 16,
              color: C.cyanDim,
              lineBreak: "noWrap",
            }}
          >
            {`/ ${spare}`}
          </text>
        </node>
      </node>
      <Rounds clip={clip} size={size} />
    </node>
  );
}

/** One pip per round in the clip (or a bar for a big clip). */
function Rounds({ clip, size }: { clip: number; size: number }) {
  if (size > MAX_PIPS) {
    return (
      <node style={{ height: 5, border: 1, borderColor: C.cyanDim }}>
        <node
          style={{
            width: `${(clip / size) * 100}%`,
            height: "100%",
            backgroundColor: C.cyan,
          }}
        />
      </node>
    );
  }
  return (
    <node style={{ flexDirection: "row", gap: 3, height: 7 }}>
      {Array.from({ length: size }, (_, i) => (
        <node
          key={i}
          style={{
            flexGrow: 1,
            height: "100%",
            backgroundColor: i < clip ? C.cyan : C.clear,
            border: 1,
            borderColor: i < clip ? C.cyanHi : C.cyanDim,
          }}
        />
      ))}
    </node>
  );
}

/** The three emitters of the sight. */
function TriDot() {
  return (
    <svg viewBox="0 0 30 8" style={{ width: 30, height: 8 }}>
      {[4, 15, 26].map((x) => (
        <circle key={x} cx={x} cy={4} r={2.6} fill={C.red} />
      ))}
    </svg>
  );
}
