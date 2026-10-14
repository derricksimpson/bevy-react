// The panel's icons: white svg glyphs served by the devtools plugin
// (`crates/core/src/devtools/assets.rs` — a Rust test pins this union to its
// table), drawn by an svg-mode `<image>` and colored by its `tint`.

export type IconName =
  | "dock"
  | "float"
  | "push"
  | "pick"
  | "overlay"
  | "paint"
  | "close"
  | "nodes"
  | "layers"
  | "animations"
  | "console"
  | "bridge"
  | "pause"
  | "play"
  | "clear"
  | "expandAll"
  | "collapseAll"
  | "check"
  | "warning";

export function Icon({
  name,
  color,
  size = 14,
  flip = false,
}: {
  name: IconName;
  color: string;
  size?: number;
  /** Mirror horizontally (the right-docked glyphs, docked left). */
  flip?: boolean;
}) {
  return (
    <image
      src={`embedded://bevy_react_core/devtools/icons/${name}.svg`}
      tint={color}
      flipX={flip}
      style={{ width: size, height: size, flexShrink: 0 }}
    />
  );
}
