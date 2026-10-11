import type { BevyStyle } from "bevy-react";
import { load, save } from "../game/act";
import { OBJECTIVES } from "../game/lore";
import { useGame } from "../game/store";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { useSpin } from "./suit/loops";

/** The save station's screen (480 × 320): SAVE writes a record of the game
 *  as it is; LOAD on a record brings it back (the room itself stays as it
 *  is). */
export function Save() {
  const saves = useGame((g) => g.fixtures.saves);
  const spin = useSpin(10);

  return (
    <Holo name="save" seed={17}>
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
            height: 54,
            flexShrink: 0,
            padding: { horizontal: 16 },
            flexDirection: "row",
            alignItems: "center",
            justifyContent: "spaceBetween",
            border: { bottom: 1 },
            borderColor: C.cyanLine,
          }}
        >
          <text
            style={{
              fontFamily: F.display,
              fontSize: 21,
              letterSpacing: 3,
              color: C.cyan,
              lineBreak: "noWrap",
            }}
          >
            SAVE STATION
          </text>
          <text style={{ ...small, color: C.cyan }}>C-7</text>
        </node>
        <node style={{ flexGrow: 1, flexDirection: "row" }}>
          <node
            style={{
              width: 172,
              flexShrink: 0,
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 16,
              border: { right: 1 },
              borderColor: C.cyanFaint,
            }}
          >
            <svg
              viewBox="0 0 90 90"
              style={{
                width: 90,
                height: 90,
                transform: { rotate: { animated: spin } },
              }}
            >
              <circle
                cx={45}
                cy={45}
                r={42}
                fill="none"
                stroke={C.cyanDim}
                strokeWidth={1.5}
              />
              <circle
                cx={45}
                cy={45}
                r={30}
                fill="none"
                stroke={C.cyan}
                strokeWidth={3}
                opacity={0.6}
              />
              <polygon
                points={[45, 13, 58, 45, 45, 77, 32, 45]}
                fill="none"
                stroke={C.white}
                strokeWidth={2}
              />
              <circle cx={45} cy={45} r={5} fill={C.cyan} />
            </svg>
            <button style={saveButton} hoverStyle={hover} onClick={save}>
              <text
                style={{
                  fontFamily: F.bold,
                  fontSize: 22,
                  letterSpacing: 4,
                  color: C.white,
                  lineBreak: "noWrap",
                }}
              >
                SAVE
              </text>
            </button>
          </node>
          <node
            style={{
              flexGrow: 1,
              flexDirection: "column",
              padding: { left: 14, right: 10, top: 10, bottom: 10 },
              gap: 8,
            }}
          >
            <text style={small}>RECORDS</text>
            <node
              style={{
                flexGrow: 1,
                flexBasis: 0,
                flexDirection: "column",
                gap: 6,
                overflowY: "scroll",
              }}
              scrollStep={30}
            >
              {saves.length === 0 ? (
                <text style={{ ...small, color: C.cyanDim }}>NO RECORDS</text>
              ) : (
                [...saves].reverse().map((s) => (
                  <node
                    key={s.id}
                    style={{
                      flexShrink: 0,
                      flexDirection: "row",
                      alignItems: "center",
                      justifyContent: "spaceBetween",
                      padding: { left: 10, right: 6, vertical: 6 },
                      backgroundColor: C.cyanFaint,
                      border: { left: 2 },
                      borderColor: C.cyan,
                    }}
                  >
                    <node style={{ flexDirection: "column", gap: 1 }}>
                      <text
                        style={{
                          fontFamily: F.bold,
                          fontSize: 19,
                          letterSpacing: 1,
                          color: C.white,
                          lineBreak: "noWrap",
                        }}
                      >
                        {`#${String(s.id).padStart(2, "0")} · ${s.time}`}
                      </text>
                      <text style={small}>{OBJECTIVES[s.objective].title}</text>
                    </node>
                    <button
                      style={loadButton}
                      hoverStyle={hover}
                      onClick={() => load(s.id)}
                    >
                      <text
                        style={{
                          fontFamily: F.bold,
                          fontSize: 16,
                          letterSpacing: 2,
                          color: C.cyan,
                          lineBreak: "noWrap",
                        }}
                      >
                        LOAD
                      </text>
                    </button>
                  </node>
                ))
              )}
            </node>
          </node>
        </node>
      </node>
    </Holo>
  );
}

const small: BevyStyle = {
  fontFamily: F.semibold,
  fontSize: 15,
  letterSpacing: 1.5,
  color: C.cyanDim,
  lineBreak: "noWrap",
};

const saveButton: BevyStyle = {
  padding: { horizontal: 26, vertical: 8 },
  border: 1,
  borderColor: C.cyan,
  backgroundColor: C.glassHi,
};

const loadButton: BevyStyle = {
  padding: { horizontal: 12, vertical: 5 },
  border: 1,
  borderColor: C.cyanLine,
  backgroundColor: C.glass,
};

const hover: BevyStyle = {
  backgroundColor: "rgba(127, 227, 255, 0.28)",
  borderColor: C.white,
};
