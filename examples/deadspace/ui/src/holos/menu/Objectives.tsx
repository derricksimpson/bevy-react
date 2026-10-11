import { OBJECTIVES } from "../../game/lore";
import { objective } from "../../game/reduce";
import { useGame } from "../../game/store";
import { C, F, T } from "../../theme";
import { Corners, Rule, Tick } from "../shared/kit";

/** What's still ahead: dark, but legible. */
const FAINT = "rgba(127, 227, 255, 0.3)";

/** The objective chain: what's done (ticked), the current one lit, what's
 *  ahead still classified — and the current one in full. */
export function Objectives() {
  const current = useGame(objective);
  const now = OBJECTIVES[current];
  return (
    <node style={{ flexDirection: "row", gap: 34, flexGrow: 1 }}>
      <node style={{ flexDirection: "column", width: 470, flexShrink: 0 }}>
        {OBJECTIVES.map((o, i) => {
          const done = i < current;
          const here = i === current;
          return (
            <node key={o.title} style={{ flexDirection: "row", gap: 16 }}>
              {/* The chain's spine: a node per step, a line between. */}
              <node
                style={{
                  flexDirection: "column",
                  alignItems: "center",
                  width: 26,
                }}
              >
                <node
                  style={{
                    width: 22,
                    height: 22,
                    borderRadius: 11,
                    border: 2,
                    borderColor: here ? C.white : done ? C.greenDim : FAINT,
                    backgroundColor: here
                      ? "rgba(240, 253, 255, 0.15)"
                      : C.clear,
                    alignItems: "center",
                    justifyContent: "center",
                  }}
                >
                  {done && <Tick size={14} />}
                </node>
                {i < OBJECTIVES.length - 1 && (
                  <node
                    style={{
                      width: 2,
                      height: 62,
                      backgroundColor: done ? C.greenDim : FAINT,
                    }}
                  />
                )}
              </node>
              <node style={{ flexDirection: "column", gap: 3 }}>
                <text
                  style={{
                    fontFamily: F.semibold,
                    fontSize: 19,
                    letterSpacing: 2,
                    color: here ? C.white : done ? C.cyanDim : FAINT,
                  }}
                >
                  {i > current ? "CLASSIFIED" : o.title}
                </text>
                <text
                  style={{
                    ...T.small,
                    color: here ? C.amber : done ? C.greenDim : FAINT,
                  }}
                >
                  {here ? "CURRENT" : done ? "COMPLETE" : "PENDING"}
                </text>
              </node>
            </node>
          );
        })}
      </node>
      <node
        style={{
          flexGrow: 1,
          flexDirection: "column",
          gap: 16,
          padding: 26,
          border: 1,
          borderColor: "rgba(127, 227, 255, 0.25)",
          backgroundColor: "rgba(4, 18, 28, 0.45)",
        }}
      >
        <Corners size={20} width={2} />
        <text style={{ ...T.small, letterSpacing: 3, color: C.amber }}>
          {`OBJECTIVE ${current + 1} / ${OBJECTIVES.length}`}
        </text>
        <text style={{ ...T.header, fontSize: 30, color: C.white }}>
          {now.title}
        </text>
        <Rule />
        <text style={{ ...T.body, fontSize: 21, lineHeight: 1.45 }}>
          {now.text}
        </text>
        <node style={{ flexGrow: 1 }} />
        <text style={{ ...T.small, letterSpacing: 2 }}>
          THE LOCATOR TRACES THE WAY: L ON THE LOCATOR BEAT
        </text>
      </node>
    </node>
  );
}
