import { BEATS } from "../../beats";
import { C, F } from "../../theme";

/** The title terminal's CONTROLS page: the tour and each beat's keys. */
export function ControlsPage({ onBack }: { onBack: () => void }) {
  return (
    <node style={{ flexGrow: 1, flexDirection: "column", gap: 14 }}>
      <node
        style={{
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "spaceBetween",
        }}
      >
        <text
          style={{
            fontFamily: F.display,
            fontSize: 24,
            letterSpacing: 6,
            color: C.white,
            lineBreak: "noWrap",
          }}
        >
          CONTROLS
        </text>
        <text
          style={{
            fontFamily: F.regular,
            fontSize: 14,
            letterSpacing: 2,
            color: C.cyanDim,
            lineBreak: "noWrap",
          }}
        >
          ← → OR 1–8 · MOVE THROUGH THE TOUR
        </text>
      </node>
      <node
        style={{
          flexGrow: 1,
          flexDirection: "row",
          flexWrap: "wrap",
          alignContent: "flexStart",
          rowGap: 10,
          columnGap: 24,
        }}
      >
        {BEATS.map((beat, i) =>
          i === 0 ? null : (
            <node
              key={beat.name}
              style={{
                width: 400,
                flexDirection: "row",
                gap: 12,
                padding: { vertical: 6, left: 10 },
                border: { left: 2 },
                borderColor: C.cyanLine,
              }}
            >
              <text
                style={{
                  width: 22,
                  fontFamily: F.display,
                  fontSize: 15,
                  color: C.cyanDim,
                  lineBreak: "noWrap",
                }}
              >
                {i}
              </text>
              <node style={{ flexDirection: "column", gap: 2 }}>
                <text
                  style={{
                    fontFamily: F.bold,
                    fontSize: 15,
                    letterSpacing: 2,
                    color: C.cyan,
                    lineBreak: "noWrap",
                  }}
                >
                  {beat.name}
                </text>
                <text
                  style={{
                    fontFamily: F.regular,
                    fontSize: 13,
                    color: C.cyanHi,
                    lineBreak: "noWrap",
                  }}
                >
                  {beat.keys
                    .map(([key, what]) => `${key} ${what}`)
                    .join("  ·  ")}
                </text>
              </node>
            </node>
          ),
        )}
      </node>
      <button
        onClick={onBack}
        style={{
          alignSelf: "flexStart",
          padding: { horizontal: 18, vertical: 7 },
          border: 1,
          borderColor: C.cyan,
          backgroundColor: C.glassHi,
        }}
        hoverStyle={{ backgroundColor: "rgba(127, 227, 255, 0.28)" }}
      >
        <text
          style={{
            fontFamily: F.bold,
            fontSize: 16,
            letterSpacing: 3,
            color: C.white,
            lineBreak: "noWrap",
          }}
        >
          BACK
        </text>
      </button>
    </node>
  );
}
