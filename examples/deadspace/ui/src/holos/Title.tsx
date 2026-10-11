import { useEffect, useState } from "react";
import type { BevyStyle } from "bevy-react";
import { bevy } from "../bevy";
import { glitch } from "../game/act";
import { useGame } from "../game/store";
import { useDebug, useKeys } from "../hooks";
import { cue } from "../sound";
import { C, F } from "../theme";
import { Holo } from "./Holo";
import { ControlsPage } from "./title/ControlsPage";

const ITEMS = ["BEGIN", "CONTROLS", "QUIT"] as const;
type Item = (typeof ITEMS)[number];

/** The title terminal in its alcove (960 × 540): the wordmark, now and then
 *  breaking up, and the menu — clickable in the world, or ↑ ↓ and Enter. */
export function Title({ onBegin }: { onBegin: () => void }) {
  const active = useGame((g) => g.beat === 0);
  const [page, setPage] = useState<"menu" | "controls">("menu");
  const [selected, setSelected] = useState(0);

  const choose = (item: Item) => {
    cue("select");
    if (item === "BEGIN") onBegin();
    else if (item === "CONTROLS") setPage("controls");
    // In the browser an exited app is a frozen page: start over.
    else if (typeof location === "undefined") bevy.app.quit(null);
    else location.reload();
  };
  const back = () => {
    cue("tab");
    setPage("menu");
  };
  const hover = (i: number) => {
    if (i !== selected) cue("tick");
    setSelected(i);
  };

  useKeys(active, (e) => {
    if (page === "controls") {
      if (["Escape", "Backspace", "Enter"].includes(e.code)) back();
      return;
    }
    if (e.code === "ArrowUp" || e.code === "KeyW") hover((selected + 2) % 3);
    else if (e.code === "ArrowDown" || e.code === "KeyS")
      hover((selected + 1) % 3);
    else if (e.code === "Enter" || e.code === "NumpadEnter")
      choose(ITEMS[selected]);
  });
  useDebug("title", (arg) => {
    if (arg === "begin") onBegin();
    else if (arg === "controls") setPage("controls");
    else if (arg === "back") setPage("menu");
  });

  // The wordmark breaks up every few seconds while the terminal is on.
  useEffect(() => {
    if (!active) return;
    let t: ReturnType<typeof setTimeout>;
    const next = () => {
      t = setTimeout(
        () => {
          glitch("title");
          next();
        },
        3000 + Math.random() * 4000,
      );
    };
    next();
    return () => clearTimeout(t);
  }, [active]);

  return (
    <Holo name="title" page={page} seed={31} style={screen}>
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
      {page === "controls" ? (
        <ControlsPage onBack={back} />
      ) : (
        <node style={{ flexGrow: 1, flexDirection: "column" }}>
          <node
            style={{
              flexGrow: 1,
              flexDirection: "column",
              justifyContent: "center",
              gap: 12,
            }}
          >
            <text
              style={{
                fontFamily: F.display,
                fontSize: 82,
                letterSpacing: 16,
                color: C.white,
                lineBreak: "noWrap",
              }}
            >
              COLDWAKE
            </text>
            <node style={{ width: 640, height: 1, backgroundColor: C.cyan }} />
            <text
              style={{
                fontFamily: F.semibold,
                fontSize: 17,
                letterSpacing: 7,
                color: C.cyan,
                lineBreak: "noWrap",
              }}
            >
              EURYDICE STATION · HALDEN ICE & ORE
            </text>
          </node>
          <node
            style={{
              flexDirection: "row",
              alignItems: "flexEnd",
              justifyContent: "spaceBetween",
            }}
          >
            <node style={{ flexDirection: "column", gap: 4 }}>
              {ITEMS.map((item, i) => (
                <button
                  key={item}
                  onClick={() => choose(item)}
                  onPointerEnter={() => hover(i)}
                  style={{
                    width: 260,
                    flexDirection: "row",
                    alignItems: "center",
                    gap: 12,
                    padding: { vertical: 6, left: 4 },
                    backgroundColor: i === selected ? C.cyanFaint : C.clear,
                  }}
                >
                  <node
                    style={{
                      width: 4,
                      height: 22,
                      backgroundColor: i === selected ? C.white : C.clear,
                    }}
                  />
                  <text
                    style={{
                      fontFamily: F.display,
                      fontSize: 20,
                      letterSpacing: 5,
                      color: i === selected ? C.white : C.cyanDim,
                      lineBreak: "noWrap",
                    }}
                  >
                    {item}
                  </text>
                </button>
              ))}
            </node>
            <text style={small}>↑ ↓ SELECT · ENTER CONFIRM</text>
          </node>
        </node>
      )}
    </Holo>
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
