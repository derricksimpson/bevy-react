import type { ReactNode } from "react";
import type { BevyStyle } from "bevy-react";
import { type IconName, Icon } from "../icons";
import { C, UI } from "../theme";

/** An editor area: a header strip (title left, tools right) over a body. */
export function Pane({
  title,
  tools,
  width,
  height,
  children,
}: {
  title: ReactNode;
  tools?: ReactNode;
  width: number;
  height: number;
  children: ReactNode;
}) {
  return (
    <node
      style={{
        width,
        height,
        flexDirection: "column",
        backgroundColor: C.pane,
        borderRadius: 4,
      }}
    >
      <node
        style={{
          height: UI.paneHeader,
          flexDirection: "row",
          alignItems: "center",
          gap: 8,
          padding: { horizontal: 10 },
          backgroundColor: C.paneHeader,
          borderRadius: { top: 4, right: 4, bottom: 0, left: 0 },
        }}
      >
        {typeof title === "string" ? (
          <text
            style={{ fontSize: UI.font, color: C.text, lineBreak: "noWrap" }}
          >
            {title}
          </text>
        ) : (
          title
        )}
        <node style={{ flexGrow: 1 }} />
        {tools}
      </node>
      <node
        style={{
          flexGrow: 1,
          flexDirection: "column",
          overflowX: "clip",
          overflowY: "clip",
        }}
      >
        {children}
      </node>
    </node>
  );
}

/** A small toggle/action button for a pane header. */
export function ToolButton({
  icon,
  label,
  active = false,
  onClick,
  style,
}: {
  icon?: IconName;
  label?: string;
  active?: boolean;
  onClick: () => void;
  style?: BevyStyle;
}) {
  return (
    <node
      style={{
        height: 20,
        minWidth: 22,
        flexDirection: "row",
        gap: 4,
        alignItems: "center",
        justifyContent: "center",
        padding: { horizontal: label ? 7 : 0 },
        borderRadius: 4,
        backgroundColor: active ? C.accent : C.field,
        ...style,
      }}
      hoverStyle={{ backgroundColor: active ? C.fillHover : C.fieldHover }}
      onClick={onClick}
    >
      {icon && <Icon name={icon} size={13} />}
      {label && (
        <text
          style={{ fontSize: UI.small, color: C.text, lineBreak: "noWrap" }}
        >
          {label}
        </text>
      )}
    </node>
  );
}
