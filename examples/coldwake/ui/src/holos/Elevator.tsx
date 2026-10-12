import type { BevyStyle } from "bevy-react";
import { call } from "../game/act";
import { useGame } from "../game/store";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { Arrow, FLOORS } from "./fixtures/floors";
import { useBlink } from "./suit/loops";

/** The freight elevator's call panel beside its door (240 × 360): a button
 *  per floor, the car's floor marked, a called floor blinking until the car
 *  arrives. */
export function Elevator() {
  const { floor, target } = useGame((g) => g.fixtures.elevator);
  const blink = useBlink(target !== null, 320, 0.25);

  return (
    <Holo name="elevator" seed={19}>
      <node
        style={{
          width: "100%",
          height: "100%",
          flexDirection: "column",
          backgroundColor: C.glass,
          border: 1,
          borderColor: C.cyanLine,
        }}
      >
        <node
          style={{
            height: 48,
            flexShrink: 0,
            flexDirection: "row",
            alignItems: "center",
            justifyContent: "center",
            gap: 10,
            border: { bottom: 1 },
            borderColor: C.cyanLine,
          }}
        >
          <text
            style={{
              fontFamily: F.display,
              fontSize: 19,
              letterSpacing: 3,
              color: C.cyan,
              lineBreak: "noWrap",
            }}
          >
            LIFT
          </text>
          <text style={{ ...small, color: C.cyan }}>C-2</text>
        </node>
        <node
          style={{
            flexGrow: 1,
            flexDirection: "column",
            gap: 6,
            padding: 8,
          }}
        >
          {[...FLOORS].reverse().map((f) => {
            const here = f.floor === floor && target === null;
            const called = f.floor === target;
            return (
              <button
                key={f.floor}
                onClick={() => call(f.floor)}
                style={{
                  flexGrow: 1,
                  flexDirection: "row",
                  alignItems: "center",
                  gap: 10,
                  padding: { horizontal: 8 },
                  border: 1,
                  borderColor: here ? C.white : called ? C.amber : C.cyanFaint,
                  backgroundColor: here ? C.glassHi : C.clear,
                  ...(called ? { opacity: { animated: blink } } : {}),
                }}
                hoverStyle={hover}
              >
                <node
                  style={{
                    width: 50,
                    height: 50,
                    flexShrink: 0,
                    alignItems: "center",
                    justifyContent: "center",
                    border: 1,
                    borderColor: here ? C.white : C.cyanDim,
                  }}
                >
                  <text
                    style={{
                      fontFamily: F.display,
                      fontSize: 28,
                      color: here ? C.white : called ? C.amber : C.cyan,
                      lineBreak: "noWrap",
                    }}
                  >
                    {f.floor}
                  </text>
                </node>
                <node style={{ flexDirection: "column" }}>
                  <text
                    style={{
                      fontFamily: F.bold,
                      fontSize: 17,
                      letterSpacing: 1,
                      color: here ? C.white : C.cyan,
                      lineBreak: "noWrap",
                    }}
                  >
                    {f.name}
                  </text>
                  {(here || called) && (
                    <text
                      style={{ ...small, color: called ? C.amber : C.white }}
                    >
                      {here ? "CAR HERE" : "CALLED"}
                    </text>
                  )}
                </node>
              </button>
            );
          })}
        </node>
        <node
          style={{
            height: 38,
            flexShrink: 0,
            flexDirection: "row",
            alignItems: "center",
            justifyContent: "center",
            gap: 8,
            border: { top: 1 },
            borderColor: C.cyanLine,
          }}
        >
          {target !== null && <Arrow up={target > floor} size={14} />}
          <text
            style={{ ...small, color: target !== null ? C.amber : C.cyanDim }}
          >
            {target !== null ? "IN TRANSIT" : "STANDING BY"}
          </text>
        </node>
      </node>
    </Holo>
  );
}

const small: BevyStyle = {
  fontFamily: F.semibold,
  fontSize: 14,
  letterSpacing: 1.5,
  color: C.cyanDim,
  lineBreak: "noWrap",
};

const hover: BevyStyle = {
  backgroundColor: "rgba(127, 227, 255, 0.2)",
  borderColor: C.cyan,
};
