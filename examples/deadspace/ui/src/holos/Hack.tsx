import { useEffect, useRef } from "react";
import type { BevyStyle } from "bevy-react";
import { useGame } from "../game/store";
import { useDebug, useKeys } from "../hooks";
import { cue } from "../sound";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { DIAL, Dial } from "./hack/Dial";
import { TUMBLERS, useBypass } from "./hack/useBypass";

/** Degrees the pick turns per A/D key event (auto-repeat while held). */
const KEY_STEP = 4;

/** The bulkhead's bypass panel (420 × 420): turn the pick (the mouse around
 *  the dial, or A/D) until the dial answers, then try (E or click). Three
 *  tumblers before the clock runs out. */
export function Hack() {
  const beat = useGame((g) => g.beat);
  const unlocked = useGame((g) => g.fixtures.door !== "locked");
  const bypass = useBypass(beat === 7, unlocked);
  const { angle, solved, time, near, done } = bypass;

  useKeys(
    beat === 7,
    (e) => {
      if (e.code === "KeyA") bypass.turn(-KEY_STEP);
      else if (e.code === "KeyD") bypass.turn(KEY_STEP);
      else if (e.code === "KeyE" && !e.repeat) bypass.attempt();
    },
    true,
  );
  useDebug("hack", (arg) => {
    const [verb, value] = arg.split(" ");
    if (verb === "solve") bypass.solve();
    else if (verb === "angle") bypass.point(Number(value));
  });

  // A faint tick each time the pick comes a step nearer.
  const step = Math.floor(near * 6);
  const last = useRef(step);
  useEffect(() => {
    if (step > last.current && step >= 3) cue("hackTick");
    last.current = step;
  }, [step]);

  const low = time < 6 && !done;
  const status = done
    ? "BYPASS COMPLETE"
    : `TUMBLER ${solved + 1} / ${TUMBLERS}`;
  return (
    <Holo
      name="hack"
      seed={29}
      style={{ flexDirection: "column", alignItems: "center" }}
    >
      <node
        style={{
          width: "100%",
          height: 52,
          flexShrink: 0,
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "spaceBetween",
          padding: { horizontal: 22 },
        }}
      >
        <text
          style={{
            fontFamily: F.display,
            fontSize: 17,
            letterSpacing: 3,
            color: done ? C.green : C.cyan,
            lineBreak: "noWrap",
          }}
        >
          BYPASS
        </text>
        <text
          style={{
            fontFamily: F.display,
            fontSize: 22,
            color: done ? C.green : low ? C.red : C.white,
            lineBreak: "noWrap",
          }}
        >
          {done ? "--.-" : time.toFixed(1).padStart(4, "0")}
        </text>
      </node>
      <node
        style={{ width: DIAL, height: DIAL }}
        onPointerHover={(e) => {
          if (done) return;
          const deg = (Math.atan2(e.x - 0.5, 0.5 - e.y) * 180) / Math.PI;
          bypass.point(deg);
        }}
        onClick={bypass.attempt}
      >
        <Dial angle={angle} solved={solved} near={near} done={done} />
      </node>
      <node
        style={{
          width: "100%",
          flexGrow: 1,
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          gap: 6,
        }}
      >
        <text
          style={{
            fontFamily: F.bold,
            fontSize: 15,
            letterSpacing: 3,
            color: done ? C.green : C.white,
            lineBreak: "noWrap",
          }}
        >
          {status}
        </text>
        {!done && (
          <node style={{ width: 240, height: 4, backgroundColor: C.cyanDeep }}>
            <node
              style={{
                width: `${near * 100}%`,
                height: "100%",
                backgroundColor: near > 0.85 ? C.white : C.cyan,
              }}
            />
          </node>
        )}
        <text style={hint}>
          {done
            ? "BULKHEAD C-D RELEASED"
            : `RESONANCE ${Math.round(near * 100)}%`}
        </text>
      </node>
    </Holo>
  );
}

const hint: BevyStyle = {
  fontFamily: F.regular,
  fontSize: 12,
  letterSpacing: 1.5,
  color: C.cyanDim,
  lineBreak: "noWrap",
};
