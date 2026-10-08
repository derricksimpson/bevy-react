import { useRef, useState } from "react";
import type { PointerEventData } from "bevy-react";
import { endTyping } from "../hooks";
import { MenuPanel } from "../menus/Menu";
import { openPopup } from "../menus/popup";
import { C, UI, hex, parseHex } from "../theme";
import { TextEdit } from "./TextEdit";

export type Rgb = [number, number, number];

export function hsvToRgb(h: number, s: number, v: number): Rgb {
  const f = (n: number) => {
    const k = (n + h * 6) % 6;
    return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
  };
  return [f(5), f(3), f(1)];
}

export function rgbToHsv([r, g, b]: Rgb): [number, number, number] {
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  let h = 0;
  if (d > 0) {
    if (max === r) h = ((g - b) / d) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
  }
  return [(h / 6 + 1) % 1, max === 0 ? 0 : d / max, max];
}

const PICKER_W = 196;
const SQUARE_H = 150;

/** An HSV picker: saturation/value square, hue strip and a hex field. Keeps
 *  its own hue so a gray (no hue) doesn't lose it. */
export function ColorPicker({
  initial,
  onChange,
}: {
  initial: Rgb;
  onChange: (rgb: Rgb, record: boolean) => void;
}) {
  const [hsv, setHsv] = useState(() => rgbToHsv(initial));
  const [editing, setEditing] = useState(false);
  const recorded = useRef(false);
  const [h, s, v] = hsv;
  const rgb = hsvToRgb(h, s, v);
  const set = (next: [number, number, number]) => {
    setHsv(next);
    onChange(hsvToRgb(...next), !recorded.current);
    recorded.current = true;
  };
  const start = () => {
    recorded.current = false;
  };
  const sv = (e: PointerEventData) => set([h, e.x, 1 - e.y]);
  const hue = (e: PointerEventData) => set([Math.min(e.x, 0.999), s, v]);
  const pure = hex(hsvToRgb(h, 1, 1));
  return (
    <MenuPanel width={PICKER_W + 16}>
      <node
        style={{
          flexDirection: "column",
          gap: 8,
          padding: { horizontal: 8, vertical: 3 },
        }}
      >
        <node
          style={{
            width: PICKER_W,
            height: SQUARE_H,
            borderRadius: 3,
            backgroundColor: pure,
            backgroundGradient: [
              {
                type: "linear",
                angle: 180,
                colorSpace: "srgb",
                stops: [{ color: "rgba(0,0,0,0)" }, { color: "#000000" }],
              },
              {
                type: "linear",
                angle: 90,
                colorSpace: "srgb",
                stops: [{ color: "#ffffff" }, { color: "rgba(255,255,255,0)" }],
              },
            ],
            cursor: "crosshair",
          }}
          onPointerDown={(e) => {
            start();
            sv(e);
          }}
          onPointerMove={sv}
        >
          <node
            style={{
              positionType: "absolute",
              left: s * PICKER_W - 6,
              top: (1 - v) * SQUARE_H - 6,
              width: 12,
              height: 12,
              borderRadius: 6,
              border: 2,
              borderColor: "#ffffff",
              backgroundColor: hex(rgb),
            }}
          />
        </node>
        <node
          style={{
            width: PICKER_W,
            height: 14,
            borderRadius: 3,
            backgroundGradient: {
              type: "linear",
              angle: 90,
              colorSpace: "srgb",
              stops: [
                "#ff0000",
                "#ffff00",
                "#00ff00",
                "#00ffff",
                "#0000ff",
                "#ff00ff",
                "#ff0000",
              ].map((color) => ({ color })),
            },
          }}
          onPointerDown={(e) => {
            start();
            hue(e);
          }}
          onPointerMove={hue}
        >
          <node
            style={{
              positionType: "absolute",
              left: h * PICKER_W - 3,
              top: -2,
              width: 6,
              height: 18,
              borderRadius: 2,
              border: 2,
              borderColor: "#ffffff",
            }}
          />
        </node>
        <node
          style={{
            flexDirection: "row",
            alignItems: "center",
            gap: 8,
            height: 22,
          }}
        >
          <node
            style={{
              width: 22,
              height: 18,
              borderRadius: 3,
              backgroundColor: hex(rgb),
            }}
          />
          <text style={{ fontSize: UI.small, color: C.textDim }}>Hex</text>
          {editing ? (
            <TextEdit
              initial={hex(rgb).slice(1)}
              style={{
                fontSize: UI.font,
                height: 18,
                padding: { horizontal: 6 },
                borderRadius: 3,
              }}
              onDone={(text) => {
                setEditing(false);
                const parsed = text === null ? null : parseHex(text);
                if (parsed) {
                  start();
                  set(rgbToHsv(parsed));
                }
              }}
            />
          ) : (
            <node
              style={{
                flexGrow: 1,
                height: 18,
                borderRadius: 3,
                backgroundColor: C.field,
                padding: { horizontal: 6 },
                justifyContent: "center",
              }}
              hoverStyle={{ backgroundColor: C.fieldHover }}
              onClick={() => setEditing(true)}
            >
              <text style={{ fontSize: UI.font, color: C.text }}>
                {hex(rgb).toUpperCase()}
              </text>
            </node>
          )}
        </node>
      </node>
    </MenuPanel>
  );
}

/** Open the picker at a press. */
export function pickColor(
  e: PointerEventData,
  initial: Rgb,
  onChange: (rgb: Rgb, record: boolean) => void,
) {
  endTyping();
  openPopup({
    x: e.clientX - 40,
    y: e.clientY + 12,
    w: PICKER_W + 16,
    h: SQUARE_H + 80,
    render: () => <ColorPicker initial={initial} onChange={onChange} />,
  });
}

/** A color swatch (with its label, when given); a press opens the picker. */
export function ColorField({
  label,
  value,
  z,
  onChange,
}: {
  label?: string;
  value: Rgb;
  z: number;
  onChange: (rgb: Rgb, record: boolean) => void;
}) {
  return (
    <node
      style={{
        flexGrow: 1,
        flexDirection: "row",
        alignItems: "center",
        gap: 6 * z,
      }}
    >
      {label && (
        <text
          style={{ fontSize: 11.5 * z, color: C.text, lineBreak: "noWrap" }}
        >
          {label}
        </text>
      )}
      <node
        style={{
          flexGrow: 1,
          height: 18 * z,
          borderRadius: 4 * z,
          border: 1,
          borderColor: "#1a1a1a",
          backgroundColor: hex(value),
        }}
        onPointerDown={(e) => {
          if (e.button === 0) pickColor(e, value, onChange);
        }}
      />
    </node>
  );
}
