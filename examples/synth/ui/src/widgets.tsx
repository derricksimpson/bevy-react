import { createContext, useContext, useEffect, useRef } from "react";
import {
  cancelAnimation,
  interpolate,
  useSharedValue,
  withRepeat,
  withTiming,
  type SharedValue,
} from "bevy-react";
import { C, FONT_MONO, alpha, r } from "./theme";

/** The transport, shared by every module: `steps` runs 0 → 16 over two
 *  bars of eighth notes while playing (Bevy drives it; the wires' gate
 *  pulses keep the same time), `spin` turns the selection rings. */
export type Live = {
  playing: boolean;
  bpm: number;
  steps: SharedValue;
  spin: SharedValue;
};
export const LiveContext = createContext<Live>(null!);
export const useLive = () => useContext(LiveContext);

/** The committed zoom: animated translations are logical px, so a scrolling
 *  preview needs it to move in graph units. */
export const ZoomContext = createContext(1);

export const STEPS = 16;
export const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** An opacity over the step loop: step `j` lights to `level(j)` (0..1) at
 *  its start and fades through it, between `dim` and 1. With `per` > 1 the
 *  steps are that many to an eighth (2: sixteenths, `j` 0..31). */
export function stepLight(
  steps: SharedValue,
  level: (j: number) => number,
  dim = 0.16,
  per = 1,
) {
  const input: number[] = [];
  const output: number[] = [];
  for (let j = 0; j < STEPS * per; j++) {
    const on = dim + (1 - dim) * level(j);
    const at = j / per;
    input.push(at, at + 0.03 / per, at + 0.92 / per);
    output.push(dim, on, dim + (on - dim) * 0.3);
  }
  input.push(STEPS);
  output.push(dim);
  return interpolate(steps, input, output);
}

/** A stable pseudo-random 0..1 per (seed, i). */
export function rand(seed: number, i: number) {
  const x = Math.sin(seed * 127.1 + i * 311.7) * 43758.5453;
  return x - Math.floor(x);
}

/** An arc of a circle, angles in degrees clockwise from up. */
export function arc(
  cx: number,
  cy: number,
  rad: number,
  a0: number,
  a1: number,
) {
  const at = (a: number) => {
    const t = (a * Math.PI) / 180;
    return `${(cx + rad * Math.sin(t)).toFixed(2)} ${(cy - rad * Math.cos(t)).toFixed(2)}`;
  };
  return `M ${at(a0)} A ${rad} ${rad} 0 ${a1 - a0 > 180 ? 1 : 0} 1 ${at(a1)}`;
}

const label = { fontSize: r(9), color: C.dim, letterSpacing: r(0.6) };

/** A knob: drag up and down (Shift for fine steps). */
export function Knob({
  name,
  value,
  onChange,
  color,
  size = 38,
  show,
}: {
  name: string;
  value: number;
  onChange: (v: number) => void;
  color: string;
  size?: number;
  show?: (v: number) => string;
}) {
  const drag = useRef<{ y: number; v: number } | null>(null);
  const angle = -135 + 270 * value;
  return (
    <node style={{ flexDirection: "column", alignItems: "center", gap: r(3) }}>
      <node
        onPointerDown={(e) => {
          drag.current = { y: e.clientY, v: value };
        }}
        onPointerMove={(e) => {
          const d = drag.current;
          if (d)
            onChange(
              clamp01(d.v + (d.y - e.clientY) / (e.shiftKey ? 700 : 170)),
            );
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
        style={{ width: r(size), height: r(size), cursor: "nsResize" }}
      >
        <svg
          viewBox="0 0 40 40"
          style={{ positionType: "absolute", width: "100%", height: "100%" }}
        >
          <path
            d={arc(20, 20, 17, -135, 135)}
            stroke={C.edge}
            strokeWidth={3}
            fill="none"
            strokeLinecap="round"
          />
          {value > 0.004 && (
            <path
              d={arc(20, 20, 17, -135, angle)}
              stroke={color}
              strokeWidth={3}
              fill="none"
              strokeLinecap="round"
            />
          )}
        </svg>
        <node
          style={{
            positionType: "absolute",
            left: "21%",
            top: "21%",
            width: "58%",
            height: "58%",
            borderRadius: "50%",
            border: r(1),
            borderColor: C.edgeHi,
            backgroundGradient: {
              type: "radial",
              position: "topLeft",
              stops: [{ color: "#3a4360" }, { color: "#1a1f2e" }],
            },
            boxShadow: {
              color: "rgba(0,0,0,0.5)",
              yOffset: r(2),
              blurRadius: r(4),
            },
            transform: { rotate: angle },
          }}
          hoverStyle={{ borderColor: color }}
        >
          <node
            style={{
              positionType: "absolute",
              left: "50%",
              top: r(2),
              width: r(2),
              height: "36%",
              margin: { left: r(-1) },
              borderRadius: r(1),
              backgroundColor: color,
            }}
          />
        </node>
      </node>
      <text style={label}>{name}</text>
      {show && (
        <text style={{ fontSize: r(9), color: C.text, fontFamily: FONT_MONO }}>
          {show(value)}
        </text>
      )}
    </node>
  );
}

/** A segmented choice with a pill that springs to the picked option. */
export function Tabs({
  options,
  value,
  onChange,
  color,
}: {
  options: string[];
  value: number;
  onChange: (i: number) => void;
  color: string;
}) {
  return (
    <node
      style={{
        height: r(22),
        padding: r(2),
        borderRadius: r(7),
        backgroundColor: C.cardDeep,
        border: r(1),
        borderColor: C.edge,
      }}
    >
      <node style={{ flexGrow: 1, flexDirection: "row" }}>
        <node
          style={{
            positionType: "absolute",
            top: 0,
            bottom: 0,
            left: 0,
            width: `${100 / options.length}%`,
            borderRadius: r(5),
            backgroundColor: alpha(color, 0.22),
            border: r(1),
            borderColor: alpha(color, 0.55),
            transform: { translateX: `${value * 100}%` },
            transition: { transform: { stiffness: 380, damping: 28 } },
          }}
        />
        {options.map((o, i) => (
          <node
            key={o}
            onClick={() => onChange(i)}
            style={{
              flexGrow: 1,
              flexBasis: 0,
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <text
              style={{
                fontSize: r(9),
                fontWeight: "semibold",
                letterSpacing: r(0.4),
                color: i === value ? C.text : C.faint,
              }}
            >
              {o}
            </text>
          </node>
        ))}
      </node>
    </node>
  );
}

/** A scope window: `cycles` periods of `wave(phase 0..1) → -1..1` across
 *  it, scrolling left one period every `periodMs` while the patch plays. */
export function Scope({
  wave,
  cycles,
  periodMs,
  color,
  width,
  height = 44,
}: {
  wave: (phase: number) => number;
  cycles: number;
  periodMs: number;
  color: string;
  /** World px. */
  width: number;
  height?: number;
}) {
  const { playing } = useLive();
  const zoom = useContext(ZoomContext);
  const t = useSharedValue(0);
  useEffect(() => {
    if (!playing) {
      cancelAnimation(t);
      return;
    }
    t.value = 0;
    t.value = withRepeat(withTiming(1, { duration: periodMs }));
  }, [t, playing, periodMs]);
  const period = width / cycles;
  // One period more than shows, so the scroll wraps seamlessly.
  const drawn = width + period;
  const samples = Math.ceil((cycles + 1) * 48);
  const mid = height / 2;
  let d = "";
  for (let i = 0; i <= samples; i++) {
    const x = (i / samples) * drawn;
    const y = mid - wave((((x / period) % 1) + 1) % 1) * (mid - 5);
    d += `${i ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`;
  }
  return (
    <node
      style={{
        width: r(width),
        height: r(height),
        overflowX: "clip",
        overflowY: "clip",
        borderRadius: r(6),
        backgroundColor: C.cardDeep,
        border: r(1),
        borderColor: C.edge,
      }}
    >
      <node
        style={{
          positionType: "absolute",
          left: 0,
          right: 0,
          top: "50%",
          height: r(1),
          backgroundColor: alpha("#ffffff", 0.05),
        }}
      />
      <svg
        viewBox={`0 0 ${drawn} ${height}`}
        style={{
          positionType: "absolute",
          left: 0,
          top: 0,
          width: r(drawn),
          height: r(height),
          transform: {
            translateX: {
              animated: interpolate(t, [0, 1], [0, -period * zoom]),
            },
          },
        }}
      >
        <path
          d={d}
          fill="none"
          stroke={alpha(color, 0.25)}
          strokeWidth={5}
          strokeLinejoin="round"
        />
        <path
          d={d}
          fill="none"
          stroke={color}
          strokeWidth={1.6}
          strokeLinejoin="round"
        />
      </svg>
    </node>
  );
}

/** An LED meter: `count` segments bouncing on the step loop, each step at
 *  a level picked from `seed`, scaled by `gain`. */
export function Meter({
  seed,
  gain,
  color,
  count = 10,
  height = 64,
  width = 8,
}: {
  seed: number;
  gain: number;
  color: string;
  count?: number;
  height?: number;
  width?: number;
}) {
  const { steps } = useLive();
  const levels = Array.from(
    { length: STEPS },
    (_, j) => gain * (0.35 + 0.65 * rand(seed, j)),
  );
  return (
    <node
      style={{
        height: r(height),
        width: r(width),
        flexDirection: "columnReverse",
        gap: r(2),
      }}
    >
      {Array.from({ length: count }, (_, k) => {
        const at = (k + 0.5) / count;
        const hue =
          k >= count - 2 ? "#ff5f7a" : k >= count - 4 ? "#ffd166" : color;
        return (
          <node
            key={k}
            style={{
              flexGrow: 1,
              borderRadius: r(1.5),
              backgroundColor: hue,
              opacity: {
                animated: stepLight(
                  steps,
                  (j) => (levels[j] >= at ? 1 : 0),
                  0.1,
                ),
              },
            }}
          />
        );
      })}
    </node>
  );
}

/** A round LED on the step loop. */
export function Led({
  level,
  color,
  size = 7,
}: {
  level: (j: number) => number;
  color: string;
  size?: number;
}) {
  const { steps } = useLive();
  return (
    <node
      style={{
        width: r(size),
        height: r(size),
        borderRadius: "50%",
        backgroundColor: color,
        boxShadow: { color: alpha(color, 0.6), blurRadius: r(6) },
        opacity: { animated: stepLight(steps, level, 0.14) },
      }}
    />
  );
}
