import { useEffect } from "react";
import {
  interpolate,
  useSharedValue,
  withRepeat,
  withTiming,
} from "bevy-react";
import { useGame } from "../game/store";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { Arrow, FLOORS } from "./fixtures/floors";

/** The floor indicator over the elevator door (240 × 80): where the car is,
 *  and an arrow running the way it goes while it moves. */
export function Floor() {
  const { floor, target } = useGame((g) => g.fixtures.elevator);
  const moving = target !== null;
  const up = moving && target > floor;
  const name = FLOORS.find((f) => f.floor === floor)?.name ?? "";

  const run = useSharedValue(0);
  useEffect(() => {
    run.value = moving
      ? withRepeat(withTiming(1, { duration: 520, easing: "easeIn" }))
      : withTiming(0, { duration: 0 });
  }, [moving, run]);

  return (
    <Holo
      name="floor"
      seed={23}
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "center",
        gap: 14,
        backgroundColor: C.glass,
        border: { top: 1, bottom: 1 },
        borderColor: C.cyanLine,
      }}
    >
      <node
        style={{
          width: 22,
          height: 40,
          alignItems: "center",
          justifyContent: "center",
          overflowX: "clip",
          overflowY: "clip",
        }}
      >
        {moving && (
          <node
            style={{
              transform: {
                translateY: {
                  animated: interpolate(
                    run,
                    [0, 1],
                    up ? [14, -14] : [-14, 14],
                  ),
                },
              },
            }}
          >
            <Arrow up={up} size={18} />
          </node>
        )}
      </node>
      <text
        style={{
          fontFamily: F.display,
          fontSize: 42,
          color: moving ? C.amber : C.white,
          lineBreak: "noWrap",
        }}
      >
        {floor}
      </text>
      <node style={{ flexDirection: "column", width: 110 }}>
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 13,
            letterSpacing: 1.5,
            color: C.cyan,
            lineBreak: "noWrap",
          }}
        >
          {name}
        </text>
        <text
          style={{
            fontFamily: F.regular,
            fontSize: 11,
            letterSpacing: 1.5,
            color: moving ? C.amber : C.cyanDim,
            lineBreak: "noWrap",
          }}
        >
          {moving ? `TO DECK ${target}` : "FREIGHT LIFT"}
        </text>
      </node>
    </Holo>
  );
}
