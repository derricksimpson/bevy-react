import { useRef, useState } from "react";
import type { PointerEventData } from "bevy-react";
import type { Field } from "../bevy";
import { endTyping } from "../hooks";
import { C } from "../theme";
import { TextEdit } from "./TextEdit";

export type FloatField = Extract<Field, { kind: "float" }>;
export type IntField = Extract<Field, { kind: "int" }>;

/** Pixels of drag before a press becomes a scrub (else it's a click: type). */
const SLOP = 3;

export function formatNumber(
  value: number,
  field: FloatField | IntField,
): string {
  if (field.kind === "int") return String(Math.round(value));
  const digits = field.step < 0.01 ? 3 : 2;
  return `${value.toFixed(digits)}${field.unit}`;
}

/** Blender's number field: drag sideways to scrub (Shift for fine steps,
 *  Ctrl to snap), click to type. A `factor` (`0..1`) draws as a filled
 *  slider and scrubs across its own width. */
export function NumberField({
  label,
  value,
  field,
  z,
  width,
  onChange,
}: {
  label: string;
  value: number;
  field: FloatField | IntField;
  /** The graph zoom (1 outside the graph). */
  z: number;
  /** The field's logical width, so a factor scrubs across it. */
  width: number;
  onChange: (value: number, record: boolean) => void;
}) {
  const [editing, setEditing] = useState(false);
  const drag = useRef({ x: 0, start: 0, scrubbing: false, recorded: false });
  const factor = field.kind === "float" && field.factor;
  const clamp = (v: number) => Math.min(field.max, Math.max(field.min, v));
  const h = 18 * z;
  const font = 11.5 * z;

  const down = (e: PointerEventData) => {
    if (e.button !== 0) return;
    endTyping();
    drag.current = {
      x: e.clientX,
      start: value,
      scrubbing: false,
      recorded: false,
    };
  };
  const move = (e: PointerEventData) => {
    const d = drag.current;
    const dx = e.clientX - d.x;
    if (!d.scrubbing && Math.abs(dx) < SLOP) return;
    d.scrubbing = true;
    let v: number;
    if (field.kind === "int") {
      v = d.start + Math.round(dx / (8 * z));
    } else {
      const perPx = factor ? (field.max - field.min) / width : field.step;
      v = d.start + (dx * perPx * (e.shiftKey ? 0.1 : 1)) / (factor ? 1 : z);
      if (e.ctrlKey) {
        const snap = factor ? 0.05 : field.step * 10;
        v = Math.round(v / snap) * snap;
      }
    }
    v = clamp(v);
    if (v !== value) {
      onChange(v, !d.recorded);
      d.recorded = true;
    }
  };
  const up = () => {
    if (!drag.current.scrubbing) setEditing(true);
  };

  if (editing) {
    return (
      <node
        style={{
          flexGrow: 1,
          height: h,
          borderRadius: 4 * z,
          overflowX: "clip",
        }}
      >
        <TextEdit
          initial={formatNumber(value, field).replace(/[^0-9.\-e]/g, "")}
          style={{
            fontSize: font,
            height: h,
            padding: { horizontal: 6 * z },
            borderRadius: 4 * z,
          }}
          onDone={(text) => {
            setEditing(false);
            const v = text === null ? NaN : parseFloat(text);
            if (Number.isFinite(v))
              onChange(clamp(field.kind === "int" ? Math.round(v) : v), true);
          }}
        />
      </node>
    );
  }

  const fill = factor ? (value - field.min) / (field.max - field.min) : 0;
  return (
    <node
      style={{
        flexGrow: 1,
        height: h,
        borderRadius: 4 * z,
        backgroundColor: C.field,
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "spaceBetween",
        padding: { horizontal: 6 * z },
        overflowX: "clip",
        cursor: "ew-resize",
      }}
      hoverStyle={{ backgroundColor: C.fieldHover }}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
    >
      {factor && (
        <node
          style={{
            positionType: "absolute",
            left: 0,
            top: 0,
            bottom: 0,
            width: `${Math.max(0, Math.min(1, fill)) * 100}%`,
            // Rounded where it meets the field's own left corners.
            borderRadius: { top: 4 * z, right: 0, bottom: 0, left: 4 * z },
            backgroundColor: C.fill,
          }}
        />
      )}
      <text style={{ fontSize: font, color: C.text, lineBreak: "noWrap" }}>
        {label}
      </text>
      <text style={{ fontSize: font, color: C.text, lineBreak: "noWrap" }}>
        {formatNumber(value, field)}
      </text>
    </node>
  );
}
