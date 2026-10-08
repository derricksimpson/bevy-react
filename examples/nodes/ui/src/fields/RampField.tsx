import { useRef, useState } from "react";
import type { PointerEventData } from "bevy-react";
import type { Stop } from "../bevy";
import { endTyping } from "../hooks";
import { Icon } from "../icons";
import { C, G, hex } from "../theme";
import { type Rgb, pickColor } from "./color";
import { NumberField } from "./NumberField";

/** The ramp's color at `t` (linear; the editor's preview of the node). */
export function sampleRamp(stops: readonly Stop[], t: number): Rgb {
  const sorted = [...stops].sort((a, b) => a.pos - b.pos);
  if (t <= sorted[0].pos) return [...sorted[0].color];
  for (let i = 1; i < sorted.length; i++) {
    const [a, b] = [sorted[i - 1], sorted[i]];
    if (t <= b.pos) {
      const f = (t - a.pos) / Math.max(b.pos - a.pos, 1e-6);
      return [0, 1, 2].map(
        (c) => a.color[c] + (b.color[c] - a.color[c]) * f,
      ) as Rgb;
    }
  }
  return [...sorted[sorted.length - 1].color];
}

/** Grab a stop within this fraction of the bar. */
const GRAB = 0.04;

/** Blender's color ramp: a gradient bar with draggable stops (press empty
 *  bar to add one), +/− buttons, and the selected stop's position and color. */
export function RampField({
  stops,
  constant,
  z,
  width,
  onChange,
}: {
  stops: Stop[];
  /** Hard steps between stops (the `constant` interpolation). */
  constant: boolean;
  z: number;
  /** The bar's logical width. */
  width: number;
  onChange: (stops: Stop[], record: boolean) => void;
}) {
  const [selected, setSelected] = useState(0);
  const sel = Math.min(selected, stops.length - 1);
  const drag = useRef({ index: -1, recorded: false });
  const sorted = [...stops].sort((a, b) => a.pos - b.pos);

  const update = (i: number, patch: Partial<Stop>, record: boolean) =>
    onChange(
      stops.map((s, j) => (j === i ? { ...s, ...patch } : s)),
      record,
    );

  const down = (e: PointerEventData) => {
    if (e.button !== 0) return;
    endTyping();
    let index = stops.findIndex((s) => Math.abs(s.pos - e.x) < GRAB);
    let recorded = false;
    if (index < 0) {
      onChange([...stops, { pos: e.x, color: sampleRamp(stops, e.x) }], true);
      index = stops.length;
      recorded = true;
    }
    setSelected(index);
    drag.current = { index, recorded };
  };
  const move = (e: PointerEventData) => {
    const d = drag.current;
    if (d.index < 0) return;
    update(d.index, { pos: Math.max(0, Math.min(1, e.x)) }, !d.recorded);
    d.recorded = true;
  };

  const gradientStops = constant
    ? sorted.flatMap((s, i) => {
        const next = sorted[i + 1]?.pos ?? 1;
        const color = hex(s.color);
        return [
          { color, position: `${s.pos * 100}%` },
          { color, position: `${next * 100}%` },
        ];
      })
    : sorted.map((s) => ({ color: hex(s.color), position: `${s.pos * 100}%` }));
  const first = sorted[0];
  const bar = 16 * z;
  const button = (icon: "plus" | "minus", onClick: () => void) => (
    <node
      style={{
        width: 18 * z,
        height: 18 * z,
        borderRadius: 4 * z,
        backgroundColor: C.field,
        alignItems: "center",
        justifyContent: "center",
      }}
      hoverStyle={{ backgroundColor: C.fieldHover }}
      onClick={onClick}
    >
      <Icon name={icon} size={10 * z} />
    </node>
  );

  return (
    <node
      style={{
        height: G.ramp * z,
        flexDirection: "column",
        justifyContent: "center",
        gap: 5 * z,
      }}
    >
      <node
        style={{ height: bar + 8 * z, flexDirection: "column" }}
        onPointerDown={down}
        onPointerMove={move}
      >
        <node
          style={{
            height: bar,
            borderRadius: 3 * z,
            border: 1,
            borderColor: "#1a1a1a",
            backgroundColor: hex(first.color),
            backgroundGradient: {
              type: "linear",
              angle: 90,
              colorSpace: "srgb",
              stops: gradientStops,
            },
          }}
        />
        {stops.map((s, i) => (
          <node
            key={i}
            style={{
              positionType: "absolute",
              left: s.pos * width - 4 * z,
              top: bar - 3 * z,
              width: 8 * z,
              height: 10 * z,
              borderRadius: 2 * z,
              border: i === sel ? 2 : 1,
              borderColor: i === sel ? "#ffffff" : "#111111",
              backgroundColor: hex(s.color),
            }}
          />
        ))}
      </node>
      <node style={{ flexDirection: "row", alignItems: "center", gap: 4 * z }}>
        {button("plus", () => {
          const at = sorted.find((s) => s.pos > stops[sel].pos)?.pos ?? 1;
          const pos = (stops[sel].pos + at) / 2;
          onChange([...stops, { pos, color: sampleRamp(stops, pos) }], true);
          setSelected(stops.length);
        })}
        {button("minus", () => {
          if (stops.length > 1)
            onChange(
              stops.filter((_, j) => j !== sel),
              true,
            );
        })}
        <NumberField
          label=""
          value={stops[sel].pos}
          field={{
            kind: "float",
            default: 0,
            min: 0,
            max: 1,
            step: 0.005,
            factor: true,
            unit: "",
          }}
          z={z}
          width={width * 0.45}
          onChange={(pos, record) => update(sel, { pos }, record)}
        />
        <node
          style={{
            width: 34 * z,
            height: 18 * z,
            borderRadius: 4 * z,
            border: 1,
            borderColor: "#1a1a1a",
            backgroundColor: hex(stops[sel].color),
          }}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            const index = sel;
            pickColor(e, stops[sel].color, (color, record) =>
              update(index, { color }, record),
            );
          }}
        />
      </node>
    </node>
  );
}
