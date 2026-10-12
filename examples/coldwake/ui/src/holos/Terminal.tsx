import { useEffect } from "react";
import type { BevyStyle } from "bevy-react";
import { glitch } from "../game/act";
import { CONTROLS } from "../keys";
import { C, F } from "../theme";
import { Holo, useLive } from "./Holo";

/** The terminal in the alcove where he starts (960 × 540): the wordmark,
 *  now and then breaking up, and every control. */
export function Terminal() {
  const live = useLive("controls");

  // The wordmark breaks up every few seconds while someone reads it.
  useEffect(() => {
    if (!live) return;
    let t: ReturnType<typeof setTimeout>;
    const next = () => {
      t = setTimeout(
        () => {
          glitch("controls");
          next();
        },
        3000 + Math.random() * 4000,
      );
    };
    next();
    return () => clearTimeout(t);
  }, [live]);

  return (
    <Holo name="controls" seed={31} style={screen}>
      <node
        style={{
          flexDirection: "row",
          justifyContent: "spaceBetween",
          border: { bottom: 1 },
          borderColor: C.cyanFaint,
          padding: { bottom: 8 },
        }}
      >
        <text style={small}>HALDEN ICE & ORE · TERMINAL C-01</text>
        <text style={small}>SHIFT 3 · CONCOURSE C</text>
      </node>
      <node
        style={{
          flexDirection: "column",
          gap: 4,
          padding: { top: 4, bottom: 12 },
          border: { bottom: 1 },
          borderColor: C.cyan,
        }}
      >
        <text
          style={{
            fontFamily: F.display,
            fontSize: 52,
            letterSpacing: 12,
            color: C.white,
            lineBreak: "noWrap",
          }}
        >
          COLDWAKE
        </text>
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 15,
            letterSpacing: 5,
            color: C.cyan,
            lineBreak: "noWrap",
          }}
        >
          EURYDICE STATION · RIG ORIENTATION · MAINTENANCE CONCOURSE C
        </text>
      </node>
      <node
        style={{
          flexGrow: 1,
          flexDirection: "row",
          flexWrap: "wrap",
          alignContent: "flexStart",
          rowGap: 18,
          columnGap: 28,
          padding: { top: 12 },
        }}
      >
        {CONTROLS.map((group) => (
          <node
            key={group.title}
            style={{
              width: 270,
              flexDirection: "column",
              gap: 6,
              padding: { left: 12 },
              border: { left: 2 },
              borderColor: C.cyanLine,
            }}
          >
            <text
              style={{
                fontFamily: F.bold,
                fontSize: 15,
                letterSpacing: 3,
                color: C.cyan,
                lineBreak: "noWrap",
              }}
            >
              {group.title}
            </text>
            {group.keys.map(([key, what]) => (
              <node
                key={key}
                style={{ flexDirection: "row", alignItems: "center", gap: 10 }}
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
        ))}
      </node>
    </Holo>
  );
}

/** A key's cap. */
export function Key({ children }: { children: string }) {
  return (
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
          color: C.white,
          lineBreak: "noWrap",
        }}
      >
        {children}
      </text>
    </node>
  );
}

const screen: BevyStyle = {
  flexDirection: "column",
  padding: { horizontal: 44, vertical: 30 },
  gap: 10,
  backgroundColor: "rgba(2, 10, 16, 0.78)",
  border: 2,
  borderColor: C.cyanLine,
};

const small: BevyStyle = {
  fontFamily: F.regular,
  fontSize: 13,
  letterSpacing: 2.5,
  color: C.cyanDim,
  lineBreak: "noWrap",
};
