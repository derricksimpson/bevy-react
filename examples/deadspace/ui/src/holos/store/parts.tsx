import type { ReactNode } from "react";
import { ITEMS, type ItemId } from "../../game/items";
import { C, F, T } from "../../theme";
import { ItemIcon, KIND_LABEL } from "../shared/items";
import { Rule } from "../shared/kit";

/** The store's list rows and its detail pane. */

/** One item row: icon, name and kind, and a value on the right. */
export function ItemRow({
  item,
  count,
  on,
  value,
  valueColor = C.amber,
  onClick,
}: {
  item: ItemId;
  count?: number;
  on: boolean;
  value: string;
  valueColor?: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        flexShrink: 0,
        flexDirection: "row",
        alignItems: "center",
        gap: 14,
        height: 74,
        padding: { horizontal: 10 },
        border: 1,
        borderColor: on ? C.white : "rgba(127, 227, 255, 0.16)",
        backgroundColor: on
          ? "rgba(240, 253, 255, 0.08)"
          : "rgba(4, 18, 28, 0.4)",
        cursor: "pointer",
      }}
      hoverStyle={{ backgroundColor: "rgba(127, 227, 255, 0.1)" }}
    >
      <ItemIcon item={item} size={60} count={count} lit={on} />
      <node
        style={{ flexDirection: "column", gap: 2, flexGrow: 1, flexShrink: 1 }}
      >
        <text
          style={{
            fontFamily: F.semibold,
            fontSize: 19,
            letterSpacing: 1.5,
            color: on ? C.white : C.cyan,
            lineBreak: "noWrap",
          }}
        >
          {ITEMS[item].name.toUpperCase()}
        </text>
        <text style={{ ...T.small, letterSpacing: 2 }}>
          {KIND_LABEL[ITEMS[item].kind]}
        </text>
      </node>
      <text style={{ fontFamily: F.display, fontSize: 19, color: valueColor }}>
        {value}
      </text>
    </button>
  );
}

/** A scrolling column of rows (the wheel works in holograms). */
export function List({
  width = 600,
  children,
}: {
  width?: number;
  children: ReactNode;
}) {
  return (
    <node
      scrollStep={40}
      style={{
        width,
        flexShrink: 0,
        height: 540,
        flexDirection: "column",
        gap: 6,
        overflowY: "scroll",
        transition: { scroll: { duration: 180, easing: "easeOut" } },
      }}
    >
      {children}
    </node>
  );
}

/** The selected item up close: the turntable (while the store is up — the
 *  `preview` camera renders only while a portal shows it), its name and
 *  text, then the page's own lines and button. */
export function Detail({
  item,
  live,
  children,
}: {
  item: ItemId | undefined;
  live: boolean;
  children?: ReactNode;
}) {
  if (!item) {
    return (
      <node
        style={{ flexGrow: 1, alignItems: "center", justifyContent: "center" }}
      >
        <text style={{ ...T.label, color: C.cyanDim, letterSpacing: 4 }}>
          NOTHING SELECTED
        </text>
      </node>
    );
  }
  return (
    <node style={{ flexGrow: 1, flexDirection: "column", gap: 12 }}>
      <node
        style={{
          width: 340,
          height: 280,
          alignSelf: "center",
          border: 1,
          borderColor: "rgba(127, 227, 255, 0.25)",
          backgroundColor: "rgba(2, 10, 16, 1)",
        }}
      >
        {live && (
          <portal
            target="preview"
            style={{ width: "100%", height: "100%", cache: "never" }}
          />
        )}
      </node>
      <text style={{ ...T.header, fontSize: 25, color: C.white }}>
        {ITEMS[item].name.toUpperCase()}
      </text>
      <Rule />
      <text style={{ ...T.body, fontSize: 19 }}>{ITEMS[item].text}</text>
      {children}
    </node>
  );
}
