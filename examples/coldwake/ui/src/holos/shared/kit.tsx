import type { ReactNode } from "react";
import type { BevyStyle } from "bevy-react";
import { C, F, T } from "../../theme";

/** The big holograms' building blocks: corner brackets, a tab bar, buttons,
 *  segmented bars, a tick. Everything is thin light on dark glass. */

/** Short bright brackets on a box's four corners (put inside a
 *  `positionType: "relative"` box). */
export function Corners({
  size = 18,
  width = 2,
  color = C.cyan,
}: {
  size?: number;
  width?: number;
  color?: string;
}) {
  const arm = (style: BevyStyle) => (
    <node
      style={{ positionType: "absolute", backgroundColor: color, ...style }}
    />
  );
  const h = { width: size, height: width };
  const v = { width, height: size };
  return (
    <>
      {arm({ ...h, left: -1, top: -1 })}
      {arm({ ...v, left: -1, top: -1 })}
      {arm({ ...h, right: -1, top: -1 })}
      {arm({ ...v, right: -1, top: -1 })}
      {arm({ ...h, left: -1, bottom: -1 })}
      {arm({ ...v, left: -1, bottom: -1 })}
      {arm({ ...h, right: -1, bottom: -1 })}
      {arm({ ...v, right: -1, bottom: -1 })}
    </>
  );
}

/** A hologram's outer shell: dark glass, a hairline frame, corner
 *  brackets. */
export function Shell({ children }: { children: ReactNode }) {
  return (
    <node
      style={{
        width: "100%",
        height: "100%",
        flexDirection: "column",
        padding: { horizontal: 28, vertical: 20 },
        backgroundColor: "rgba(3, 14, 22, 0.78)",
        border: 1,
        borderColor: C.cyanLine,
      }}
    >
      <Corners size={34} width={3} />
      {children}
    </node>
  );
}

/** A horizontal hairline. */
export function Rule({ color = C.cyanLine }: { color?: string }) {
  return <node style={{ height: 1, flexShrink: 0, backgroundColor: color }} />;
}

/** A row of tabs; the active one is white over a lit underline. */
export function Tabs<T extends string>({
  tabs,
  active,
  onPick,
  disabled = [],
}: {
  tabs: readonly { id: T; label: string }[];
  active: T;
  onPick: (id: T) => void;
  disabled?: readonly T[];
}) {
  return (
    <node style={{ flexDirection: "row", gap: 6 }}>
      {tabs.map(({ id, label }) => {
        const on = id === active;
        const off = disabled.includes(id);
        return (
          <button
            key={id}
            onClick={() => !off && onPick(id)}
            style={{
              padding: { horizontal: 22, vertical: 8 },
              border: { bottom: 2 },
              borderColor: on ? C.cyan : C.clear,
              backgroundColor: on ? C.cyanFaint : C.clear,
              cursor: off ? "default" : "pointer",
            }}
            hoverStyle={
              off ? undefined : { backgroundColor: "rgba(127, 227, 255, 0.09)" }
            }
          >
            <text
              style={{
                ...T.label,
                letterSpacing: 2.5,
                color: off ? C.cyanDeep : on ? C.white : C.cyanDim,
              }}
            >
              {label}
            </text>
          </button>
        );
      })}
    </node>
  );
}

/** A framed text button. `tone` colors its line and label. */
export function HoloButton({
  label,
  onClick,
  tone = C.cyan,
  disabled = false,
  wide = false,
}: {
  label: string;
  onClick: () => void;
  tone?: string;
  disabled?: boolean;
  wide?: boolean;
}) {
  const color = disabled ? C.cyanDeep : tone;
  return (
    <button
      onClick={onClick}
      style={{
        padding: { horizontal: 28, vertical: 9 },
        minWidth: wide ? 220 : 0,
        justifyContent: "center",
        border: 1,
        borderColor: color,
        backgroundColor: "rgba(4, 18, 28, 0.5)",
        cursor: disabled ? "default" : "pointer",
      }}
      hoverStyle={
        disabled ? undefined : { backgroundColor: "rgba(127, 227, 255, 0.18)" }
      }
      pressStyle={
        disabled ? undefined : { backgroundColor: "rgba(127, 227, 255, 0.34)" }
      }
    >
      <text
        style={{
          fontFamily: F.semibold,
          fontSize: 19,
          letterSpacing: 3,
          color,
          lineBreak: "noWrap",
        }}
      >
        {label}
      </text>
    </button>
  );
}

/** A bar of `segments` cells, `value` (0..1) of them lit. */
export function Bar({
  value,
  segments = 10,
  color = C.cyan,
  width = 180,
  height = 9,
}: {
  value: number;
  segments?: number;
  color?: string;
  width?: number;
  height?: number;
}) {
  const lit = Math.round(value * segments);
  return (
    <node style={{ flexDirection: "row", gap: 2, width, height }}>
      {Array.from({ length: segments }, (_, i) => (
        <node
          key={i}
          style={{
            flexGrow: 1,
            backgroundColor: i < lit ? color : "rgba(127, 227, 255, 0.12)",
          }}
        />
      ))}
    </node>
  );
}

/** A small tick (the fonts have no ✓). */
export function Tick({
  color = C.green,
  size = 16,
}: {
  color?: string;
  size?: number;
}) {
  return (
    <svg viewBox="0 0 16 16" style={{ width: size, height: size }}>
      <polyline
        points={[2.5, 8.5, 6.5, 12.5, 13.5, 3.5]}
        fill="none"
        stroke={color}
        strokeWidth={2.2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
