import { useEffect, useState } from "react";
import { interpolate, useSharedValue, withSpring } from "bevy-react";
import { bevy } from "../bevy";
import { CATEGORIES, type Category, type Kind, KINDS } from "../graph/catalog";
import { Icon } from "../graph/NodeCard";
import { useLatest, useWindowSize } from "../hooks";
import { C, alpha, glass } from "../theme";

const WIDTH = 240;
const ITEM = 34;
const HEAD = 22;

/** Whether a menu is open: the app's single-key shortcuts step aside while
 *  it takes typing. */
let open = 0;
export const menuOpen = () => open > 0;

/** The Add menu: every module, or only those a dropped wire fits. Typing
 *  filters, Enter takes the first, Escape closes (the canvas). */
export function AddMenu({
  x,
  y,
  accepts,
  onPick,
  onClose,
}: {
  x: number;
  y: number;
  accepts?: (k: Kind) => boolean;
  onPick: (k: Kind) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const size = useWindowSize();
  const t = useSharedValue(0);
  useEffect(() => {
    t.value = withSpring(1, { stiffness: 420, damping: 28 });
  }, [t]);

  const kinds = (Object.keys(KINDS) as Kind[]).filter(
    (k) =>
      (!accepts || accepts(k)) &&
      KINDS[k].title.toLowerCase().includes(query.toLowerCase()),
  );
  const groups = (Object.keys(CATEGORIES) as Category[])
    .map((c) => [c, kinds.filter((k) => KINDS[k].category === c)] as const)
    .filter(([, ks]) => ks.length);

  const latest = useLatest({ kinds, onPick, onClose });
  useEffect(() => {
    open++;
    const off = bevy.on("keyDown", (e) => {
      const { kinds, onPick, onClose } = latest.current;
      if (e.key === "Backspace") setQuery((q) => q.slice(0, -1));
      else if (e.key === "Enter") {
        if (kinds[0]) onPick(kinds[0]);
        else onClose();
      } else if (e.text && /^[\w ]$/.test(e.text) && !e.ctrlKey) {
        setQuery((q) => q + e.text);
      }
    });
    return () => {
      open--;
      off();
    };
  }, [latest]);

  const height =
    44 + groups.reduce((h, [, ks]) => h + HEAD + ks.length * ITEM, 0);
  const left = Math.max(8, Math.min(x, size.width - WIDTH - 8));
  const top = Math.max(8, Math.min(y, size.height - height - 8));

  return (
    <node
      style={{
        ...glass,
        positionType: "absolute",
        left,
        top,
        width: WIDTH,
        flexDirection: "column",
        padding: 6,
        globalZIndex: 20,
        opacity: { animated: t },
        transform: {
          scale: { animated: interpolate(t, [0, 1, 2], [0.92, 1, 1.08]) },
        },
      }}
    >
      <node
        style={{
          height: 30,
          flexDirection: "row",
          alignItems: "center",
          gap: 8,
          padding: { left: 8 },
          margin: { bottom: 4 },
          borderRadius: 8,
          backgroundColor: "rgba(0, 0, 0, 0.25)",
        }}
      >
        <Icon
          d="M10.5 17a6.5 6.5 0 1 1 0-13a6.5 6.5 0 0 1 0 13Z M15.5 15.5 20 20"
          color={C.dim}
          size={14}
        />
        <text style={{ fontSize: 12, color: query ? C.text : C.faint }}>
          {query || (accepts ? "Plug into…" : "Add a module…")}
        </text>
      </node>
      {groups.map(([c, ks]) => (
        <node key={c} style={{ flexDirection: "column" }}>
          <text
            style={{
              height: HEAD,
              padding: { left: 8, top: 6 },
              fontSize: 10,
              fontWeight: "semibold",
              letterSpacing: 1,
              color: CATEGORIES[c].color,
            }}
          >
            {CATEGORIES[c].label.toUpperCase()}
          </text>
          {ks.map((k) => (
            <node
              key={k}
              onClick={() => onPick(k)}
              style={{
                height: ITEM,
                flexDirection: "row",
                alignItems: "center",
                gap: 10,
                padding: { left: 8 },
                borderRadius: 8,
                backgroundColor:
                  k === kinds[0] && query
                    ? alpha("#ffffff", 0.06)
                    : "transparent",
                transition: { backgroundColor: { duration: 120 } },
              }}
              hoverStyle={{ backgroundColor: alpha(CATEGORIES[c].color, 0.16) }}
            >
              <Icon d={KINDS[k].icon} color={CATEGORIES[c].color} size={16} />
              <node style={{ flexGrow: 1, flexDirection: "column" }}>
                <text style={{ fontSize: 12, color: C.text }}>
                  {KINDS[k].title}
                </text>
                <text style={{ fontSize: 10, color: C.faint }}>
                  {KINDS[k].blurb}
                </text>
              </node>
            </node>
          ))}
        </node>
      ))}
    </node>
  );
}
