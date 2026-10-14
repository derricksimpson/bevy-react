import { type ReactNode, useRef, useState } from "react";
import { Icon } from "../graph/NodeCard";
import type { Preset } from "../graph/presets";
import { C, FONT_MONO, SIGNAL, alpha, glass } from "../theme";
import { Presets } from "./Presets";

const button = {
  height: 32,
  flexDirection: "row",
  alignItems: "center",
  gap: 7,
  padding: { left: 11, right: 12 },
  borderRadius: 9,
  backgroundColor: "rgba(255, 255, 255, 0)",
  transition: {
    backgroundColor: { duration: 120 },
    transform: { stiffness: 500, damping: 26 },
  },
  transform: { scale: 1 },
} as const;

function Button({
  icon,
  label,
  onClick,
}: {
  icon: string;
  label: string;
  onClick: () => void;
}) {
  return (
    <node
      onClick={onClick}
      style={button}
      hoverStyle={{ backgroundColor: "rgba(255, 255, 255, 0.07)" }}
      pressStyle={{ transform: { scale: 0.94 } }}
    >
      <Icon d={icon} color={C.dim} size={15} />
      <text style={{ fontSize: 12, color: C.text }}>{label}</text>
    </node>
  );
}

const Divider = () => (
  <node style={{ width: 1, height: 22, backgroundColor: C.glassEdge }} />
);

export function TopBar({
  presets,
  preset,
  onPreset,
  playing,
  onPlay,
  bpm,
  onBpm,
  zoom,
  onArrange,
  onFit,
}: {
  presets: Preset[];
  preset: number;
  onPreset: (i: number) => void;
  playing: boolean;
  onPlay: () => void;
  bpm: number;
  onBpm: (bpm: number) => void;
  zoom: number;
  onArrange: () => void;
  onFit: () => void;
}) {
  return (
    <node
      style={{
        positionType: "absolute",
        left: 0,
        right: 0,
        top: 14,
        justifyContent: "center",
      }}
    >
      <node
        style={{
          ...glass,
          height: 48,
          flexDirection: "row",
          alignItems: "center",
          gap: 10,
          padding: { left: 14, right: 8 },
        }}
      >
        <Logo />
        <text
          style={{
            fontSize: 15,
            fontWeight: "bold",
            color: C.text,
            letterSpacing: 0.5,
          }}
        >
          Synth
        </text>
        <Divider />
        <Presets presets={presets} value={preset} onChange={onPreset} />
        <Divider />
        <Play playing={playing} onPlay={onPlay} />
        <Tempo bpm={bpm} onBpm={onBpm} />
        <Divider />
        <Button
          icon="M4 6h6v4H4z M14 14h6v4h-6z M10 8h2v8h2"
          label="Arrange"
          onClick={onArrange}
        />
        <Button
          icon="M4 9V4h5 M20 9V4h-5 M4 15v5h5 M20 15v5h-5"
          label="Fit"
          onClick={onFit}
        />
        <text
          style={{
            width: 52,
            fontSize: 12,
            fontFamily: FONT_MONO,
            color: C.dim,
            textAlign: "right",
          }}
        >
          {`${Math.round(zoom * 100)}%`}
        </text>
      </node>
    </node>
  );
}

function Logo() {
  return (
    <svg viewBox="0 0 28 28" style={{ width: 26, height: 26 }}>
      <path
        d="M7 8 C14 8 14 20 21 20"
        fill="none"
        stroke={SIGNAL.audio}
        strokeWidth={2.2}
      />
      <path
        d="M7 20 C12 20 14 14 21 14"
        fill="none"
        stroke={SIGNAL.gate}
        strokeWidth={2.2}
      />
      <circle cx={7} cy={8} r={3.4} fill={SIGNAL.audio} />
      <circle cx={7} cy={20} r={3.4} fill={SIGNAL.gate} />
      <circle cx={21} cy={20} r={3.4} fill={C.text} />
      <circle cx={21} cy={14} r={3.4} fill={SIGNAL.cv} />
    </svg>
  );
}

function Play({ playing, onPlay }: { playing: boolean; onPlay: () => void }) {
  const color = playing ? SIGNAL.audio : C.text;
  return (
    <node
      onClick={onPlay}
      style={{
        width: 34,
        height: 34,
        borderRadius: 17,
        alignItems: "center",
        justifyContent: "center",
        border: 1,
        borderColor: alpha(SIGNAL.audio, playing ? 0.6 : 0.25),
        backgroundColor: alpha(SIGNAL.audio, playing ? 0.16 : 0.05),
        // The icon cross-fades between play and pause.
        morphFilter: { key: playing ? 1 : 0, name: "crossfade" },
        transform: { scale: 1 },
        transition: {
          transform: { stiffness: 500, damping: 24 },
          morphFilter: { duration: 220 },
        },
      }}
      pressStyle={{ transform: { scale: 0.9 } }}
    >
      {playing ? (
        <svg viewBox="0 0 24 24" style={{ width: 14, height: 14 }}>
          <rect x={5} y={4} width={4.5} height={16} rx={1.2} fill={color} />
          <rect x={14.5} y={4} width={4.5} height={16} rx={1.2} fill={color} />
        </svg>
      ) : (
        <svg viewBox="0 0 24 24" style={{ width: 14, height: 14 }}>
          <path d="M7 4.5v15l12-7.5z" fill={color} />
        </svg>
      )}
    </node>
  );
}

/** The tempo: drag sideways to scrub; it takes effect on release. */
function Tempo({ bpm, onBpm }: { bpm: number; onBpm: (bpm: number) => void }) {
  const [live, setLive] = useState<number | null>(null);
  const drag = useRef<{ x: number; bpm: number } | null>(null);
  const shown = live ?? bpm;
  const scrub = (x: number) => {
    const d = drag.current;
    return d
      ? Math.max(60, Math.min(180, Math.round(d.bpm + (x - d.x) / 3)))
      : bpm;
  };
  return (
    <node
      onPointerDown={(e) => {
        drag.current = { x: e.clientX, bpm };
      }}
      onPointerMove={(e) => setLive(scrub(e.clientX))}
      onPointerUp={(e) => {
        const next = scrub(e.clientX);
        drag.current = null;
        setLive(null);
        if (next !== bpm) onBpm(next);
      }}
      style={{
        height: 32,
        flexDirection: "row",
        alignItems: "center",
        gap: 6,
        padding: { horizontal: 10 },
        borderRadius: 9,
        cursor: "ewResize",
        backgroundColor:
          live === null ? "rgba(0, 0, 0, 0.22)" : "rgba(0, 0, 0, 0.4)",
      }}
      hoverStyle={{ backgroundColor: "rgba(0, 0, 0, 0.4)" }}
    >
      <text style={{ fontSize: 15, fontFamily: FONT_MONO, color: C.text }}>
        {shown}
      </text>
      <text style={{ fontSize: 10, color: C.dim }}>BPM</text>
    </node>
  );
}

export function Hints() {
  const items: [string, string][] = [
    ["Drag a socket", "patch"],
    ["Right-click", "add"],
    ["WASD", "pan"],
    ["Space", "play"],
    ["F", "fit"],
    ["Del", "remove"],
    ["Shift-drag", "select"],
  ];
  return (
    <node
      style={{
        ...glass,
        positionType: "absolute",
        left: 14,
        bottom: 14,
        height: 34,
        flexDirection: "row",
        alignItems: "center",
        gap: 14,
        padding: { horizontal: 14 },
        borderRadius: 17,
      }}
    >
      {items.map(([key, what]) => (
        <Hint key={key} k={key} what={what} />
      ))}
    </node>
  );
}

function Hint({ k, what }: { k: string; what: string }): ReactNode {
  return (
    <node style={{ flexDirection: "row", alignItems: "center", gap: 6 }}>
      <text
        style={{
          fontSize: 10,
          fontFamily: FONT_MONO,
          color: C.text,
          padding: "1px 5px",
          borderRadius: 4,
          backgroundColor: "rgba(255, 255, 255, 0.08)",
        }}
      >
        {k}
      </text>
      <text style={{ fontSize: 11, color: C.dim }}>{what}</text>
    </node>
  );
}
