import { useRef } from "react";
import { useGame } from "../game/store";
import { C, F } from "../theme";
import { hint, type Hint } from "./hint";
import { Holo } from "./Holo";
import { Key } from "./Terminal";

/** The line of light beside his right arm (460 × 48): what to press where
 *  he stands (`hint`). Folded while it has nothing to say. */
export function Prompt() {
  // (`hint` builds a new object each call: not a `useGame` selector.)
  const now = hint(useGame());
  // Folding away, it keeps its last line.
  const last = useRef<Hint | null>(null);
  if (now) last.current = now;
  const shown = now ?? last.current;

  return (
    <Holo
      name="prompt"
      page={shown?.id ?? "none"}
      seed={23}
      style={{ alignItems: "center", justifyContent: "center" }}
    >
      {/* A plate of dark glass under the line: it has to read over
          whatever stands behind it, a lit panel included. */}
      {shown && (
        <node
          style={{
            flexDirection: "row",
            alignItems: "center",
            gap: 20,
            padding: { horizontal: 14, vertical: 6 },
            backgroundColor: C.shade,
            border: 1,
            borderColor: C.cyanFaint,
          }}
        >
          {shown.keys.map(([key, what]) => (
            <node
              key={key}
              style={{ flexDirection: "row", alignItems: "center", gap: 8 }}
            >
              <Key>{key}</Key>
              <text
                style={{
                  fontFamily: F.regular,
                  fontSize: 16,
                  color: C.cyanHi,
                  lineBreak: "noWrap",
                }}
              >
                {what}
              </text>
            </node>
          ))}
        </node>
      )}
    </Holo>
  );
}
