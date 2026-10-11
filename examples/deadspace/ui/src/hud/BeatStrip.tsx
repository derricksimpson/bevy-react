import { BEATS } from "../beats";
import { C, F } from "../theme";

/** The only flat UI: the tour's beats along the bottom of the screen (click
 *  one, or ← → and 1–8) and the keys the current beat answers. */
export function BeatStrip({
  beat,
  onBeat,
}: {
  beat: number;
  onBeat: (beat: number) => void;
}) {
  return (
    <node
      style={{
        positionType: "absolute",
        left: 0,
        right: 0,
        bottom: 28,
        flexDirection: "column",
        alignItems: "center",
        gap: 10,
      }}
    >
      <node style={{ flexDirection: "row", gap: 22, alignItems: "center" }}>
        {BEATS[beat].keys.map(([key, what]) => (
          <node
            key={key}
            style={{ flexDirection: "row", gap: 8, alignItems: "center" }}
          >
            <node
              style={{
                padding: { horizontal: 7, vertical: 1 },
                border: 1,
                borderColor: "rgba(127, 227, 255, 0.55)",
                borderRadius: 3,
              }}
            >
              <text
                style={{
                  fontFamily: F.semibold,
                  fontSize: 15,
                  color: C.cyanHi,
                }}
              >
                {key}
              </text>
            </node>
            <text
              style={{
                fontFamily: F.regular,
                fontSize: 16,
                color: "rgba(196, 246, 255, 0.75)",
              }}
            >
              {what}
            </text>
          </node>
        ))}
      </node>
      <node style={{ flexDirection: "row", gap: 4 }}>
        {BEATS.map((b, i) =>
          i === 0 ? null : (
            <button
              key={b.name}
              onClick={() => onBeat(i)}
              style={{
                flexDirection: "row",
                gap: 7,
                alignItems: "center",
                padding: { horizontal: 12, vertical: 5 },
                backgroundColor:
                  i === beat
                    ? "rgba(127, 227, 255, 0.16)"
                    : "rgba(2, 8, 12, 0.45)",
                border: { bottom: 2 },
                borderColor: i === beat ? C.cyan : "rgba(127, 227, 255, 0.18)",
                cursor: "pointer",
              }}
              hoverStyle={{ backgroundColor: "rgba(127, 227, 255, 0.1)" }}
            >
              <text
                style={{
                  fontFamily: F.display,
                  fontSize: 11,
                  color: C.cyanDim,
                }}
              >
                {i}
              </text>
              <text
                style={{
                  fontFamily: F.semibold,
                  fontSize: 15,
                  letterSpacing: 1.5,
                  color: i === beat ? C.white : C.cyanDim,
                }}
              >
                {b.name}
              </text>
            </button>
          ),
        )}
      </node>
    </node>
  );
}
