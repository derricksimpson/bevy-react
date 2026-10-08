import { useState } from "react";
import { Icon } from "../icons";
import { C, UI } from "../theme";

export interface MenuItem {
  label: string;
  /** A shortcut, right-aligned. */
  hint?: string;
  disabled?: boolean;
  checked?: boolean;
  onSelect?: () => void;
  /** A submenu, opened on hover. */
  items?: MenuItem[];
  /** A thin line above this item. */
  separator?: boolean;
  /** A dot in this color before the label (a node's category). */
  swatch?: string;
}

export const MENU_ROW = 22;
export const MENU_WIDTH = 210;

/** The height a menu of `items` takes. */
export const menuHeight = (items: MenuItem[]) =>
  items.length * MENU_ROW + items.filter((i) => i.separator).length * 7 + 10;

export function MenuPanel({
  children,
  width = MENU_WIDTH,
}: {
  children: React.ReactNode;
  width?: number;
}) {
  return (
    <node
      style={{
        width,
        flexDirection: "column",
        padding: { vertical: 5 },
        backgroundColor: C.menu,
        border: 1,
        borderColor: C.menuBorder,
        borderRadius: 6,
        boxShadow: { color: "rgba(0,0,0,0.5)", blurRadius: 12, yOffset: 4 },
      }}
    >
      {children}
    </node>
  );
}

/** A dropdown/context menu; `close` closes the whole popup. */
export function Menu({
  items,
  close,
  width = MENU_WIDTH,
  title,
}: {
  items: MenuItem[];
  close: () => void;
  width?: number;
  title?: string;
}) {
  return (
    <MenuPanel width={width}>
      {title && <MenuTitle>{title}</MenuTitle>}
      <MenuItems items={items} close={close} width={width} />
    </MenuPanel>
  );
}

export function MenuTitle({ children }: { children: string }) {
  return (
    <text
      style={{
        fontSize: UI.small,
        color: C.textMuted,
        padding: { horizontal: 10, bottom: 4, top: 1 },
      }}
    >
      {children}
    </text>
  );
}

/** A menu's rows; a hovered row with `items` opens its submenu. */
export function MenuItems({
  items,
  close,
  width = MENU_WIDTH,
}: {
  items: MenuItem[];
  close: () => void;
  width?: number;
}) {
  const [open, setOpen] = useState<number | null>(null);
  return (
    <>
      {items.map((item, i) => (
        <node key={`${i}-${item.label}`} style={{ flexDirection: "column" }}>
          {item.separator && (
            <node
              style={{
                height: 1,
                margin: { vertical: 3, horizontal: 6 },
                backgroundColor: "#2e2e2e",
              }}
            />
          )}
          <MenuRow
            item={item}
            width={width}
            open={open === i}
            onHover={() => setOpen(item.items ? i : null)}
            close={close}
          />
        </node>
      ))}
    </>
  );
}

export function MenuRow({
  item,
  width,
  open,
  onHover,
  close,
}: {
  item: MenuItem;
  width: number;
  open: boolean;
  onHover: () => void;
  close: () => void;
}) {
  const color = item.disabled ? C.textMuted : C.text;
  return (
    <node
      style={{
        height: MENU_ROW,
        flexDirection: "row",
        alignItems: "center",
        padding: { horizontal: 10 },
        gap: 6,
        margin: { horizontal: 4 },
        borderRadius: 3,
        backgroundColor: open ? C.hover : "transparent",
      }}
      hoverStyle={item.disabled ? undefined : { backgroundColor: C.hover }}
      onPointerEnter={onHover}
      onClick={() => {
        if (item.disabled || item.items) return;
        close();
        item.onSelect?.();
      }}
    >
      <node style={{ width: 12, alignItems: "center" }}>
        {item.checked && <Icon name="check" size={11} />}
        {item.swatch && (
          <node
            style={{
              width: 8,
              height: 8,
              borderRadius: 4,
              backgroundColor: item.swatch,
            }}
          />
        )}
      </node>
      <text
        style={{ fontSize: UI.font, color, flexGrow: 1, lineBreak: "noWrap" }}
      >
        {item.label}
      </text>
      {item.hint && (
        <text
          style={{
            fontSize: UI.small,
            color: C.textMuted,
            lineBreak: "noWrap",
          }}
        >
          {item.hint}
        </text>
      )}
      {item.items && <Icon name="chevronRight" size={10} color={C.textDim} />}
      {item.items && open && (
        <node style={{ positionType: "absolute", left: width - 12, top: -6 }}>
          <Menu items={item.items} close={close} width={width} />
        </node>
      )}
    </node>
  );
}
