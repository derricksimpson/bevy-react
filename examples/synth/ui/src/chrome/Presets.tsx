import { useEffect, useState } from "react";
import { bevy } from "../bevy";
import { Icon } from "../graph/NodeCard";
import { C, FONT_MONO, SIGNAL, alpha, glass } from "../theme";

type Item = { name: string; blurb: string; bpm: number };

/** The preset picker: the patch's name between back and forward; the name
 *  opens the list. */
export function Presets({
  presets,
  value,
  onChange,
}: {
  presets: Item[];
  value: number;
  onChange: (i: number) => void;
}) {
  const [open, setOpen] = useState(false);
  const step = (by: number) =>
    onChange((value + by + presets.length) % presets.length);
  useEffect(() => {
    if (!open) return;
    return bevy.on("keyDown", (e) => {
      if (e.key === "Escape") setOpen(false);
    });
  }, [open]);
  return (
    <node style={{ flexDirection: "row", alignItems: "center", gap: 2 }}>
      <Arrow d="M15 6l-6 6 6 6" onClick={() => step(-1)} />
      <node
        onClick={() => setOpen((o) => !o)}
        style={{
          width: 150,
          height: 32,
          flexDirection: "row",
          alignItems: "center",
          gap: 6,
          padding: { left: 12, right: 9 },
          borderRadius: 9,
          backgroundColor: open
            ? "rgba(255, 255, 255, 0.1)"
            : "rgba(0, 0, 0, 0.28)",
          transition: { backgroundColor: { duration: 120 } },
        }}
        hoverStyle={{ backgroundColor: "rgba(255, 255, 255, 0.08)" }}
      >
        <text style={{ flexGrow: 1, fontSize: 13, color: C.text }}>
          {presets[value].name}
        </text>
        <Icon d="M6 9l6 6 6-6" color={C.dim} size={14} />
      </node>
      <Arrow d="M9 6l6 6-6 6" onClick={() => step(1)} />
      {open && (
        <List
          presets={presets}
          value={value}
          onPick={(i) => {
            setOpen(false);
            onChange(i);
          }}
          onClose={() => setOpen(false)}
        />
      )}
    </node>
  );
}

function Arrow({ d, onClick }: { d: string; onClick: () => void }) {
  return (
    <node
      onClick={onClick}
      style={{
        width: 26,
        height: 32,
        alignItems: "center",
        justifyContent: "center",
        borderRadius: 8,
        backgroundColor: "rgba(255, 255, 255, 0)",
        transition: { backgroundColor: { duration: 120 } },
      }}
      hoverStyle={{ backgroundColor: "rgba(255, 255, 255, 0.07)" }}
    >
      <Icon d={d} color={C.dim} size={15} />
    </node>
  );
}

/** Every preset, under the top bar. A press anywhere else closes it; the
 *  list is `fixed`, so the bar's frosted layer doesn't clip it. */
function List({
  presets,
  value,
  onPick,
  onClose,
}: {
  presets: Item[];
  value: number;
  onPick: (i: number) => void;
  onClose: () => void;
}) {
  return (
    <node
      onPointerDown={onClose}
      style={{
        positionType: "fixed",
        left: 0,
        right: 0,
        top: 0,
        bottom: 0,
        flexDirection: "column",
        alignItems: "center",
        globalZIndex: 40,
      }}
    >
      <node
        // Owns the presses between the rows, so they don't close it.
        onPointerDown={() => {}}
        style={{
          ...glass,
          margin: { top: 70 },
          width: 340,
          flexDirection: "column",
          padding: 6,
        }}
      >
        {presets.map((p, i) => (
          <node
            key={p.name}
            onClick={() => onPick(i)}
            style={{
              flexDirection: "row",
              alignItems: "center",
              gap: 10,
              padding: { horizontal: 10, vertical: 7 },
              borderRadius: 8,
              backgroundColor:
                i === value ? alpha("#ffffff", 0.08) : alpha("#ffffff", 0),
              transition: { backgroundColor: { duration: 120 } },
            }}
            hoverStyle={{ backgroundColor: alpha("#ffffff", 0.12) }}
          >
            <node style={{ flexGrow: 1, flexDirection: "column", gap: 2 }}>
              <text
                style={{
                  fontSize: 13,
                  color: i === value ? SIGNAL.audio : C.text,
                }}
              >
                {p.name}
              </text>
              <text style={{ fontSize: 11, color: C.dim }}>{p.blurb}</text>
            </node>
            <text style={{ fontSize: 11, fontFamily: FONT_MONO, color: C.dim }}>
              {`${p.bpm}`}
            </text>
          </node>
        ))}
      </node>
    </node>
  );
}
