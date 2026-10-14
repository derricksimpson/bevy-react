import type { ReactNode } from "react";
import { C, SIGNAL, alpha, r } from "../theme";
import { Knob, Tabs, rand, stepLight } from "../widgets";
import { type BodyProps, Col, Row, pct } from "./kit";

/** The drum machine: the beat's grid of hits and the drum voice. */

const LANES = ["A", "B", "C", "D"];

/** Lights sixteenth `i` of the bar (the transport runs two bars: `j` is
 *  its sixteenth, 0..31). */
const sixteenth = (i: number) => (j: number) => (j % 16 === i ? 1 : 0);

export function Beat({ node, set, color, steps }: BodyProps) {
  return (
    <Col gap={5}>
      {LANES.map((name, lane) => {
        const mask = node.params[`k${lane}`];
        return (
          <Lane key={name} name={name}>
            {(i) => {
              const on = ((mask >> i) & 1) === 1;
              return (
                <Pad
                  key={i}
                  on={on}
                  downbeat={i % 4 === 0}
                  color={color}
                  onClick={() => set(`k${lane}`, mask ^ (1 << i))}
                  glow={on ? stepLight(steps, sixteenth(i), 0, 2) : null}
                />
              );
            }}
          </Lane>
        );
      })}
      <Lane name="">
        {(i) => (
          <node
            key={i}
            style={{
              flexGrow: 1,
              flexBasis: 0,
              height: r(3),
              borderRadius: r(1.5),
              backgroundColor: i % 4 ? C.dim : "#ffffff",
              opacity: { animated: stepLight(steps, sixteenth(i), 0.15, 2) },
            }}
          />
        )}
      </Lane>
    </Col>
  );
}

/** A lane's name, then its sixteen in fours. */
function Lane({
  name,
  children,
}: {
  name: string;
  children: (i: number) => ReactNode;
}) {
  return (
    <node style={{ flexDirection: "row", alignItems: "center", gap: r(6) }}>
      <text
        style={{
          width: r(9),
          fontSize: r(9),
          fontWeight: "semibold",
          color: C.dim,
        }}
      >
        {name}
      </text>
      <node style={{ flexGrow: 1, flexDirection: "row", gap: r(6) }}>
        {[0, 1, 2, 3].map((beat) => (
          <node
            key={beat}
            style={{
              flexGrow: 1,
              flexBasis: 0,
              flexDirection: "row",
              alignItems: "center",
              gap: r(2.5),
            }}
          >
            {[0, 1, 2, 3].map((k) => children(beat * 4 + k))}
          </node>
        ))}
      </node>
    </node>
  );
}

/** A step of a lane: click to toggle; a hit flashes as it plays. */
function Pad({
  on,
  downbeat,
  color,
  onClick,
  glow,
}: {
  on: boolean;
  downbeat: boolean;
  color: string;
  onClick: () => void;
  glow: ReturnType<typeof stepLight> | null;
}) {
  return (
    <node
      onClick={onClick}
      style={{
        flexGrow: 1,
        flexBasis: 0,
        height: r(17),
        borderRadius: r(3),
        backgroundColor: on
          ? alpha(color, 0.7)
          : downbeat
            ? "#171c2a"
            : C.cardDeep,
        border: r(1),
        borderColor: on ? color : C.edge,
      }}
      hoverStyle={{ borderColor: C.text }}
    >
      {glow && (
        <node
          style={{
            flexGrow: 1,
            borderRadius: r(2),
            backgroundColor: "#ffffff",
            opacity: { animated: glow },
          }}
        />
      )}
    </node>
  );
}

const VOICES = ["KICK", "SNARE", "CLAP", "HAT"];

/** Each voice's main decay, seconds, as `synth/drums.rs` has it. */
const DECAYS = [
  (d: number) => 0.08 + d * d * 0.7,
  (d: number) => 0.05 + d * d * 0.3,
  (d: number) => 0.05 + d * d * 0.4,
  (d: number) => 0.012 + d * d * 0.35,
];

/** What `tune` does, per voice: the pitch it sets. */
const PITCH = [
  (v: number) => `${Math.round(40 * 2 ** (v * 1.25))}`,
  (v: number) => `${Math.round(160 * 2 ** v)}`,
  (v: number) => `${Math.round(900 * 2 ** (v * 1.5))}`,
  (v: number) => `${(9 * 2 ** (v - 0.5)).toFixed(1)}k`,
];

export function Drum({ node, set, color }: BodyProps) {
  const { voice, tune, decay, level } = node.params;
  return (
    <Col>
      <Hit voice={voice} decay={decay} level={level} />
      <Tabs
        options={VOICES}
        value={voice}
        onChange={(i) => set("voice", i)}
        color={color}
      />
      <Row gap={16}>
        <Knob
          name="TUNE"
          value={tune}
          onChange={(v) => set("tune", v)}
          color={color}
          size={32}
          show={PITCH[voice]}
        />
        <Knob
          name="DECAY"
          value={decay}
          onChange={(v) => set("decay", v)}
          color={color}
          size={32}
          show={(v) => `${Math.round(DECAYS[voice](v) * 1000)}ms`}
        />
        <Knob
          name="LEVEL"
          value={level}
          onChange={(v) => set("level", v)}
          color={color}
          size={32}
          show={pct}
        />
      </Row>
    </Col>
  );
}

/** A drawing of one hit over 0.6 s: a kick's slowing swing, a snare's tone
 *  in noise, a clap's slaps, a hat's hiss. */
function Hit({
  voice,
  decay,
  level,
}: {
  voice: number;
  decay: number;
  level: number;
}) {
  const W = 212;
  const H = 40;
  const N = 240;
  const tau = DECAYS[voice](decay);
  const env = (t: number) => {
    if (voice !== 2) return Math.exp(-t / tau);
    const slaps = [0, 0.011, 0.022].map((a) =>
      t >= a ? Math.exp(-(t - a) / 0.0045) : 0,
    );
    const tail = t >= 0.024 ? 0.7 * Math.exp(-(t - 0.024) / tau) : 0;
    return Math.max(tail, ...slaps);
  };
  let line = "";
  for (let i = 0; i <= N; i++) {
    const t = (i / N) * 0.6;
    const noise = rand(13 + voice, i) * 2 - 1;
    const v = [
      Math.sin(Math.PI * 2 * 7 * (1 - Math.exp(-t * 3))),
      0.5 * Math.sin(Math.PI * 2 * 24 * t) + 0.6 * noise,
      noise,
      noise,
    ][voice];
    const y =
      H / 2 -
      Math.max(-1, Math.min(1, v)) * env(t) * (H / 2 - 4) * (0.4 + 0.6 * level);
    line += `${i ? "L" : "M"}${((i / N) * W).toFixed(1)} ${y.toFixed(1)}`;
  }
  return (
    <node
      style={{
        height: r(H),
        borderRadius: r(6),
        backgroundColor: C.cardDeep,
        border: r(1),
        borderColor: C.edge,
      }}
    >
      <svg viewBox={`0 0 ${W} ${H}`} style={{ width: "100%", height: "100%" }}>
        <path
          d={line}
          fill="none"
          stroke={SIGNAL.audio}
          strokeWidth={1.2}
          strokeLinejoin="round"
        />
      </svg>
    </node>
  );
}
