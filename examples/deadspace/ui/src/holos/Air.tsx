import { airMax } from "../game/reduce";
import { useGame } from "../game/store";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { useBlink } from "./suit/loops";

/** Under this many seconds the countdown turns red and blinks. */
const LOW = 15;

/** "0:42" from seconds (rounded up: 0:00 means the tanks are dry). */
function clock(secs: number): string {
  const s = Math.ceil(secs);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** The air countdown over the left shoulder while the room is in vacuum
 *  (220 × 110). */
export function Air() {
  return (
    <Holo name="air" seed={7}>
      <AirTimer />
    </Holo>
  );
}

function AirTimer() {
  const air = useGame((g) => g.suit.air);
  const max = useGame(airMax);
  const low = air < LOW;
  const color = low ? C.red : C.cyan;
  const blink = useBlink(low, 280, 0.25);

  return (
    <node
      style={{
        width: "100%",
        height: "100%",
        padding: { left: 14, right: 12, top: 8, bottom: 10 },
        flexDirection: "column",
        justifyContent: "spaceBetween",
        backgroundColor: C.glass,
        border: { left: 3, top: 1, right: 1, bottom: 1 },
        borderColor: low ? C.redDim : C.cyanLine,
        opacity: { animated: blink },
      }}
    >
      <node style={{ flexDirection: "row", justifyContent: "spaceBetween" }}>
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 13,
            letterSpacing: 2.5,
            color,
            lineBreak: "noWrap",
          }}
        >
          O2 SUPPLY
        </text>
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 13,
            letterSpacing: 2.5,
            color: C.red,
            lineBreak: "noWrap",
          }}
        >
          VACUUM
        </text>
      </node>
      <text
        style={{
          fontFamily: F.display,
          fontSize: 38,
          color: low ? C.red : C.white,
          lineBreak: "noWrap",
        }}
      >
        {clock(air)}
      </text>
      <node
        style={{
          height: 6,
          border: 1,
          borderColor: low ? C.redDim : C.cyanDim,
        }}
      >
        <node
          style={{
            width: `${Math.max(0, Math.min(1, air / max)) * 100}%`,
            height: "100%",
            backgroundColor: color,
          }}
        />
      </node>
    </node>
  );
}
