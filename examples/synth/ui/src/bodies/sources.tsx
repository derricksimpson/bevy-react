import { SIGNAL } from "../theme";
import { Knob, Meter, Scope, Tabs, rand } from "../widgets";
import { type BodyProps, Col, Row, pct } from "./kit";

/** The sources that draw their signal in a scope: oscillator, noise, LFO. */

const TAU = Math.PI * 2;
const WAVES = [
  (p: number) => Math.sin(p * TAU),
  (p: number) => 2 * p - 1,
  (p: number) => (p < 0.5 ? 0.8 : -0.8),
  (p: number) => 1 - 4 * Math.abs(p - 0.5),
];

export function Oscillator({ node, set, color }: BodyProps) {
  const { wave, tune, level, detune } = node.params;
  const semis = Math.round(tune * 24 - 12);
  return (
    <Col>
      <Scope
        wave={WAVES[wave]}
        cycles={2 + Math.round(tune * 4)}
        periodMs={420 - tune * 200}
        color={SIGNAL.audio}
        width={212}
      />
      <Tabs
        options={["SIN", "SAW", "SQR", "TRI"]}
        value={wave}
        onChange={(i) => set("wave", i)}
        color={color}
      />
      <Row gap={18}>
        <Knob
          name="TUNE"
          value={tune}
          onChange={(v) => set("tune", v)}
          color={color}
          show={() => `${semis > 0 ? "+" : ""}${semis}`}
        />
        <Knob
          name="DETUNE"
          value={detune}
          onChange={(v) => set("detune", v)}
          color={color}
          // Cents each way (`Osc::step` in `synth/voices.rs`).
          show={(v) => `${Math.round(v * v * 35)}c`}
        />
        <Knob
          name="LEVEL"
          value={level}
          onChange={(v) => set("level", v)}
          color={color}
          show={pct}
        />
      </Row>
    </Col>
  );
}

export function Noise({ node, set, color }: BodyProps) {
  const { color: tint, level } = node.params;
  // Fewer, smoother points the darker the noise.
  const points = [48, 18, 8][tint];
  const noise = (p: number) => {
    const at = p * points;
    const i = Math.floor(at);
    const f = at - i;
    const v = (k: number) => rand(7 + tint, k % points) * 2 - 1;
    return (
      (v(i) + (v(i + 1) - v(i)) * f * f * (3 - 2 * f)) * (0.35 + 0.6 * level)
    );
  };
  return (
    <Col>
      <Scope
        wave={noise}
        cycles={1}
        periodMs={900}
        color={SIGNAL.audio}
        width={188}
        height={40}
      />
      <Tabs
        options={["WHITE", "PINK", "BROWN"]}
        value={tint}
        onChange={(i) => set("color", i)}
        color={color}
      />
      <Row gap={18}>
        <Knob
          name="LEVEL"
          value={level}
          onChange={(v) => set("level", v)}
          color={color}
          show={pct}
        />
        <Meter
          seed={3 + tint}
          gain={level}
          color={SIGNAL.audio}
          height={52}
          count={8}
        />
      </Row>
    </Col>
  );
}

/** The LFO's rate in Hz: 0.02 to 20, logarithmic (`Lfo::step`). */
const hz = (rate: number) => 0.02 * 1000 ** rate;

export function Lfo({ node, set, color }: BodyProps) {
  const { shape, rate, depth } = node.params;
  const scale = 0.25 + 0.75 * depth;
  // Random steps hold a level a cycle each: four of them across the scope.
  const steps = [0.6, -0.8, 0.2, -0.3].map(
    (v, i) => v + rand(9, i) * 0.4 - 0.2,
  );
  const wave =
    shape === 3
      ? (p: number) => steps[Math.min(3, Math.floor(p * 4))] * scale
      : (p: number) => WAVES[[0, 3, 2][shape]](p) * scale;
  const period = 1000 / hz(rate);
  return (
    <Col>
      <Scope
        wave={wave}
        cycles={1}
        periodMs={Math.min(
          9000,
          Math.max(300, shape === 3 ? period * 4 : period),
        )}
        color={SIGNAL.cv}
        width={188}
      />
      <Tabs
        options={["SIN", "TRI", "SQR", "S&H"]}
        value={shape}
        onChange={(i) => set("shape", i)}
        color={color}
      />
      <Row gap={22}>
        <Knob
          name="RATE"
          value={rate}
          onChange={(v) => set("rate", v)}
          color={color}
          show={(v) => {
            const f = hz(v);
            return f < 1 ? f.toFixed(2) : f < 10 ? f.toFixed(1) : f.toFixed(0);
          }}
        />
        <Knob
          name="DEPTH"
          value={depth}
          onChange={(v) => set("depth", v)}
          color={color}
          show={pct}
        />
      </Row>
    </Col>
  );
}
