// The panel's chrome: the header (layout and mode toggles) and the tab bar.

import { useState } from "react";
import type { PointerEventData } from "../jsx";
import { Icon, type IconName } from "./Icon";
import { theme } from "./theme";

export type Tab = "nodes" | "layers" | "animations" | "console" | "bridge";

/** Panel layout: docked to a window edge, or a floating window. */
export type Mode = "left" | "right" | "float";

/** A pointer drag's handlers (`useDragDelta` in `DevtoolsHost.tsx`). */
export type DragHandlers = {
  onPointerDown: (e: PointerEventData) => void;
  onPointerMove: (e: PointerEventData) => void;
  onPointerUp: () => void;
};

// Below this panel width the tab bar shows only the active tab's label.
const COMPACT_TABS_WIDTH = 470;

export function Header({
  picking,
  overlay,
  paintFlash,
  mode,
  reserve,
  drag,
  onMode,
  onReserve,
  onPick,
  onOverlay,
  onPaintFlash,
  onClose,
  toggleKey,
}: {
  picking: boolean;
  overlay: boolean;
  paintFlash: boolean;
  mode: Mode;
  reserve: boolean;
  /** Float mode: drag handlers that move the window. Safe to put on the whole
   *  header — a press on a child button attributes to the button (buttons
   *  block), so button clicks never start a window drag. */
  drag?: DragHandlers;
  onMode: (m: Mode) => void;
  onReserve: () => void;
  onPick: () => void;
  onOverlay: () => void;
  onPaintFlash: () => void;
  onClose: () => void;
  /** The configured toggle key, once Bevy has said (the Close tooltip). */
  toggleKey?: string;
}) {
  return (
    <node
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "spaceBetween",
        padding: { left: 10, right: 6, top: 6, bottom: 6 },
        backgroundColor: theme.bgAlt,
        border: { bottom: 1 },
        borderColor: theme.border,
        // Belt-and-braces vs the scroll-clip picking bug (see core
        // `pick_clip`): fixed sections block hits so scrolled-out tree rows
        // can never be reached through them even if the core filter regresses.
        focusPolicy: "block",
        // Over the sections below it, the tab bar included: its tooltips
        // overlap them.
        zIndex: 2,
      }}
      {...(drag ?? {})}
    >
      <text style={{ color: theme.text, fontSize: 13 }}>devtools</text>
      <node style={{ flexDirection: "row", alignItems: "center", gap: 2 }}>
        <HeaderButton
          icon="dock"
          flip
          title="Dock left"
          active={mode === "left"}
          onClick={() => onMode("left")}
        />
        <HeaderButton
          icon="dock"
          title="Dock right"
          active={mode === "right"}
          onClick={() => onMode("right")}
        />
        <HeaderButton
          icon="float"
          title="Float"
          active={mode === "float"}
          onClick={() => onMode("float")}
        />
        {mode !== "float" && (
          <HeaderButton
            icon="push"
            flip={mode === "left"}
            title="Push the app aside"
            active={reserve}
            onClick={onReserve}
          />
        )}
        <Divider />
        <HeaderButton
          icon="pick"
          title="Pick a node on screen"
          active={picking}
          onClick={onPick}
        />
        <HeaderButton
          icon="overlay"
          title="Outline the selection"
          active={overlay}
          onClick={onOverlay}
        />
        <HeaderButton
          icon="paint"
          title="Paint flashing"
          active={paintFlash}
          onClick={onPaintFlash}
        />
        <Divider />
        <HeaderButton
          icon="close"
          title="Close"
          hint={toggleKey}
          onClick={onClose}
        />
      </node>
    </node>
  );
}

/** A vertical hairline between header button groups. */
function Divider() {
  return (
    <node
      style={{
        width: 1,
        height: 14,
        margin: { horizontal: 4 },
        backgroundColor: theme.border,
      }}
    />
  );
}

/** An icon-only header toggle; `title` shows as a tooltip on hover. */
function HeaderButton({
  icon,
  title,
  hint,
  active = false,
  flip,
  onClick,
}: {
  icon: IconName;
  title: string;
  hint?: string;
  active?: boolean;
  flip?: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      style={{
        width: 24,
        height: 24,
        alignItems: "center",
        justifyContent: "center",
        borderRadius: 4,
        backgroundColor: active ? theme.bgActive : "transparent",
      }}
      hoverStyle={{ backgroundColor: theme.bgActive }}
      onClick={onClick}
      onPointerEnter={() => setHover(true)}
      onPointerLeave={() => setHover(false)}
    >
      <Icon
        name={icon}
        flip={flip}
        size={15}
        color={active ? theme.accent : hover ? theme.text : theme.textDim}
      />
      <Tooltip text={title} hint={hint} show={hover} side="right" />
    </button>
  );
}

/** A label floating below its parent button while `show`. Mount it inside
 *  the button: it is toggled by `display`, so the label stays in the tree
 *  (tests find buttons by it), and hover is direct-hit, so the pointer
 *  leaving the button for the tooltip hides it. Its section must stack above
 *  the ones below (`zIndex`). */
export function Tooltip({
  text,
  hint,
  show,
  side,
}: {
  text: string;
  /** A dim aside after the text: the action's key. */
  hint?: string;
  show: boolean;
  /** Which of the button's edges it lines up with. */
  side: "left" | "right";
}) {
  return (
    <node
      style={{
        display: show ? "flex" : "none",
        positionType: "absolute",
        top: "100%",
        [side]: 0,
        margin: { top: 4 },
        padding: { horizontal: 6, vertical: 3 },
        borderRadius: 4,
        backgroundColor: theme.tooltip,
        border: 1,
        borderColor: theme.border,
        boxShadow: { color: "#00000066", yOffset: 2, blurRadius: 6 },
      }}
    >
      <text style={{ color: theme.text, fontSize: 11, lineBreak: "noWrap" }}>
        {text}
      </text>
      {hint && (
        <text
          style={{
            color: theme.textDim,
            fontSize: 11,
            margin: { left: 6 },
            lineBreak: "noWrap",
          }}
        >
          {hint}
        </text>
      )}
    </node>
  );
}

const TABS: { tab: Tab; label: string; icon: IconName }[] = [
  { tab: "nodes", label: "Nodes", icon: "nodes" },
  { tab: "layers", label: "Layers", icon: "layers" },
  { tab: "animations", label: "Animations", icon: "animations" },
  { tab: "console", label: "Console", icon: "console" },
  { tab: "bridge", label: "Bridge", icon: "bridge" },
];

export function TabBar({
  tab,
  onTab,
  width,
}: {
  tab: Tab;
  onTab: (t: Tab) => void;
  /** The panel's width: too narrow for every label, only the active tab
   *  shows its own. */
  width: number;
}) {
  const compact = width < COMPACT_TABS_WIDTH;
  return (
    <node
      style={{
        flexDirection: "row",
        backgroundColor: theme.bgAlt,
        border: { bottom: 1 },
        borderColor: theme.border,
        focusPolicy: "block",
        // Over the sections below it, which its tooltips overlap.
        zIndex: 1,
      }}
    >
      {TABS.map((t) => (
        <TabButton
          key={t.tab}
          label={t.label}
          icon={t.icon}
          active={tab === t.tab}
          compact={compact}
          onClick={() => onTab(t.tab)}
        />
      ))}
    </node>
  );
}

function TabButton({
  label,
  icon,
  active,
  compact,
  onClick,
}: {
  label: string;
  icon: IconName;
  active: boolean;
  compact: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  const inline = active || !compact;
  return (
    <button
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 5,
        padding: { horizontal: 10, vertical: 6 },
        border: { bottom: 2 },
        borderColor: active ? theme.accent : "transparent",
      }}
      hoverStyle={{ backgroundColor: theme.bgActive }}
      onClick={onClick}
      onPointerEnter={() => setHover(true)}
      onPointerLeave={() => setHover(false)}
    >
      <Icon
        name={icon}
        size={13}
        color={active ? theme.accent : hover ? theme.text : theme.textDim}
      />
      {inline ? (
        <text
          style={{ color: active ? theme.text : theme.textDim, fontSize: 12 }}
        >
          {label}
        </text>
      ) : (
        <Tooltip text={label} show={hover} side="left" />
      )}
    </button>
  );
}
