import { useEffect, useRef, useState } from "react";
import { bevy, type NodeDef } from "../bevy";
import { allDefs } from "../catalog";
import { startTyping } from "../hooks";
import { Icon } from "../icons";
import { C, CATEGORIES, CATEGORY, UI } from "../theme";
import {
  MENU_ROW,
  MENU_WIDTH,
  MenuItems,
  MenuPanel,
  MenuRow,
  MenuTitle,
} from "./Menu";

/** The Add menu: categories with their nodes, and a search that takes over
 *  as soon as anything is typed (Enter adds the highlighted result).
 *  `accepts` narrows it to nodes a dragged wire can connect to. */
export function AddMenu({
  close,
  onPick,
  accepts,
}: {
  close: () => void;
  onPick: (kind: string) => void;
  accepts?: (d: NodeDef) => boolean;
}) {
  const [query, setQuery] = useState("");
  const [hi, setHi] = useState(0);
  const defs = allDefs().filter((d) => !accepts || accepts(d));
  const q = query.trim().toLowerCase();
  const results = q
    ? defs
        .filter(
          (d) =>
            d.label.toLowerCase().includes(q) ||
            d.kind.toLowerCase().includes(q),
        )
        .sort(
          (a, b) =>
            +!a.label.toLowerCase().startsWith(q) -
            +!b.label.toLowerCase().startsWith(q),
        )
        .slice(0, 12)
    : [];

  const pick = (kind: string) => {
    close();
    onPick(kind);
  };
  const latest = useRef({ results, hi, pick });
  latest.current = { results, hi, pick };

  useEffect(() => {
    // The search owns the keyboard while the menu is open; a press outside
    // closes the menu (the popup layer) before it could commit anything.
    const stop = startTyping(() => {});
    const off = bevy.on("keyDown", (e) => {
      const { results, hi, pick } = latest.current;
      if (e.key === "ArrowDown")
        setHi((h) => Math.min(h + 1, results.length - 1));
      else if (e.key === "ArrowUp") setHi((h) => Math.max(h - 1, 0));
      else if (e.key === "Enter" && results[hi]) pick(results[hi].kind);
    });
    return () => {
      stop();
      off();
    };
  }, []);

  const categories = CATEGORIES.map((c) => ({
    category: c,
    items: defs.filter((d) => d.category === c),
  })).filter((c) => c.items.length);

  return (
    <MenuPanel>
      <MenuTitle>{accepts ? "Add & Connect" : "Add"}</MenuTitle>
      <node
        style={{
          height: MENU_ROW,
          margin: { horizontal: 6, bottom: 4 },
          padding: { horizontal: 6 },
          gap: 6,
          flexDirection: "row",
          alignItems: "center",
          borderRadius: 4,
          backgroundColor: C.fieldDark,
          border: 1,
          borderColor: C.accent,
        }}
      >
        <Icon name="search" size={11} color={C.textMuted} />
        <editableText
          autofocus
          value={query}
          onChange={(v) => {
            setQuery(v);
            setHi(0);
          }}
          style={{ flexGrow: 1, fontSize: UI.font, color: C.text }}
        />
      </node>
      {q ? (
        results.length ? (
          results.map((d, i) => (
            <MenuRow
              key={d.kind}
              item={{
                label: d.label,
                hint: CATEGORY[d.category].label,
                swatch: CATEGORY[d.category].header,
                onSelect: () => onPick(d.kind),
              }}
              width={MENU_WIDTH}
              open={i === hi}
              onHover={() => setHi(i)}
              close={close}
            />
          ))
        ) : (
          <text
            style={{
              fontSize: UI.font,
              color: C.textMuted,
              padding: { horizontal: 12, vertical: 4 },
            }}
          >
            No matches
          </text>
        )
      ) : (
        <MenuItems
          close={close}
          items={categories.map(({ category, items }) => ({
            label: CATEGORY[category].label,
            swatch: CATEGORY[category].header,
            items: items.map((d) => ({
              label: d.label,
              onSelect: () => onPick(d.kind),
            })),
          }))}
        />
      )}
    </MenuPanel>
  );
}

/** The Add menu's height, for placing it. */
export const ADD_MENU_HEIGHT = MENU_ROW * 10 + 40;
