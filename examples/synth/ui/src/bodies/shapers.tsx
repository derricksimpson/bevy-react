import type { ReactNode } from "react";
import { C, SIGNAL, alpha, r } from "../theme";
import { Knob, Tabs, stepLight } from "../widgets";
import { type BodyProps, Col, Row, pct } from "./kit";

/** The modules that shape a signal, each drawing what it does: the
 *  envelope, the filter's curve, the VCA's response, the glide's slew. */

/** A curve in a recessed box: `line` and the area under it. */
function Plot({
  w,
  h,
  line,
  color,
  children,
}: {
  w: number;
  h: number;
  line: string;
  color: string;
  children?: ReactNode;
}) {
  return (
    <node
      style={{
        height: r(h),
        borderRadius: r(6),
        backgroundColor: C.cardDeep,
        border: r(1),
        borderColor: C.edge,
      }}
    >
      <svg viewBox={`0 0 ${w} ${h}`} style={{ width: "100%", height: "100%" }}>
        <path d={`${line} L${w} ${h} L0 ${h} Z`} fill={alpha(color, 0.13)} />
        <path
          d={line}
          fill="none"
          stroke={color}
          strokeWidth={1.8}
          strokeLinejoin="round"
        />
      </svg>
      {children}
    </node>
  );
}

/** `f(x)` (x and the result 0..1) as a path across a `w`×`h` plot. */
function trace(w: number, h: number, f: (x: number) => number, n = 64) {
  let d = "";
  for (let i = 0; i <= n; i++) {
    const x = i / n;
    const y = h - 4 - Math.min(1.2, f(x)) * (h - 10);
    d += `${i ? "L" : "M"}${(x * w).toFixed(1)} ${y.toFixed(1)}`;
  }
  return d;
}

export function Envelope({ node, set, color, steps }: BodyProps) {
  const { a, d, s, r: rel } = node.params;
  const W = 220;
  const H = 50;
  const top = 5;
  const floor = H - 3;
  const quarter = W / 4;
  const x1 = 2 + a * quarter;
  const x2 = x1 + 6 + d * quarter;
  const ys = floor - s * (floor - top);
  const x3 = x2 + quarter * 0.8;
  const x4 = Math.min(W - 2, x3 + 6 + rel * quarter);
  const line = `M2 ${floor} L${x1} ${top} Q${x1 + (x2 - x1) * 0.3} ${ys} ${x2} ${ys} L${x3} ${ys} Q${x3 + (x4 - x3) * 0.3} ${floor} ${x4} ${floor}`;
  const knobs: [string, string][] = [
    ["a", "ATK"],
    ["d", "DEC"],
    ["s", "SUS"],
    ["r", "REL"],
  ];
  return (
    <Col>
      <node
        style={{
          height: r(H),
          borderRadius: r(6),
          backgroundColor: C.cardDeep,
          border: r(1),
          borderColor: C.edge,
        }}
      >
        <svg
          viewBox={`0 0 ${W} ${H}`}
          style={{ width: "100%", height: "100%" }}
        >
          <path
            d={`${line} L${x4} ${H} L2 ${H} Z`}
            fill={alpha(SIGNAL.cv, 0.14)}
          />
          <path
            d={line}
            fill="none"
            stroke={SIGNAL.cv}
            strokeWidth={1.8}
            strokeLinejoin="round"
          />
        </svg>
        <node
          style={{
            positionType: "absolute",
            left: 0,
            top: 0,
            bottom: 0,
            width: r(x1 + 4),
            borderRadius: r(5),
            backgroundColor: alpha(SIGNAL.cv, 0.22),
            opacity: { animated: stepLight(steps, () => 1, 0) },
          }}
        />
      </node>
      <Row gap={8}>
        {knobs.map(([key, name]) => (
          <Knob
            key={key}
            name={name}
            value={node.params[key]}
            onChange={(v) => set(key, v)}
            color={color}
            size={30}
          />
        ))}
      </Row>
    </Col>
  );
}

export function Filter({ node, set, color }: BodyProps) {
  const { mode, cutoff, res, mod } = node.params;
  const W = 212;
  const H = 50;
  // The resonant peak grows with Q (0.5 to 25, as the engine maps it).
  const q = 0.5 * 50 ** res;
  const bump = 0.5 * Math.log10(Math.max(1, q / 0.707));
  const line = trace(W, H, (x) => {
    const f = Math.pow(10, (x - cutoff) * 2.2);
    const lp = 1 / Math.sqrt(1 + f ** 4);
    const hp = 1 / Math.sqrt(1 + f ** -4);
    const peak = bump * Math.exp(-(((x - cutoff) / 0.05) ** 2));
    return [lp, Math.min(lp, hp) * 1.6, hp][mode] + peak;
  });
  return (
    <Col>
      <Plot w={W} h={H} line={line} color={SIGNAL.audio}>
        <node
          style={{
            positionType: "absolute",
            left: `${cutoff * 100}%`,
            top: r(4),
            bottom: r(4),
            width: r(1),
            backgroundColor: alpha(color, 0.5),
          }}
        />
      </Plot>
      <Tabs
        options={["LOW", "BAND", "HIGH"]}
        value={mode}
        onChange={(i) => set("mode", i)}
        color={color}
      />
      <Row gap={18}>
        <Knob
          name="CUTOFF"
          value={cutoff}
          onChange={(v) => set("cutoff", v)}
          color={color}
          show={(v) => `${Math.round(20 * 1000 ** v)}`}
        />
        <Knob
          name="RES"
          value={res}
          onChange={(v) => set("res", v)}
          color={color}
          show={pct}
        />
        <Knob
          name="MOD"
          value={mod}
          onChange={(v) => set("mod", v)}
          color={color}
          // How far the `cutoff` input opens it, octaves.
          show={(v) => `${(v * 6).toFixed(1)}`}
        />
      </Row>
    </Col>
  );
}

export function Vca({ node, set, color }: BodyProps) {
  const { gain, curve } = node.params;
  const line = trace(188, 44, (x) => (curve ? x ** 3 : x) * gain);
  return (
    <Col>
      <Plot w={188} h={44} line={line} color={SIGNAL.audio} />
      <Row gap={16}>
        <node style={{ width: r(96), flexDirection: "column" }}>
          <Tabs
            options={["LIN", "EXP"]}
            value={curve}
            onChange={(i) => set("curve", i)}
            color={color}
          />
        </node>
        <Knob
          name="GAIN"
          value={gain}
          onChange={(v) => set("gain", v)}
          color={color}
          size={34}
          show={pct}
        />
      </Row>
    </Col>
  );
}

/** The glide's time constant, seconds (`Dsp::Glide` in `synth/engine.rs`). */
const slew = (time: number) => 0.001 + time * time * 0.5;

export function Glide({ node, set, color }: BodyProps) {
  const { time } = node.params;
  const W = 176;
  const H = 40;
  // Five notes a fifth of a second each: the steps as played, and slewed.
  const notes = [0.2, 0.8, 0.45, 0.95, 0.3];
  const at = (x: number) => notes[Math.min(4, Math.floor(x * 5))];
  let level = notes[0];
  const smooth = trace(W, H, (x) => {
    level += (at(x) - level) * (1 - Math.exp(-(1 / 64) / slew(time)));
    return level;
  });
  const stepped = trace(W, H, at, 320);
  return (
    <Col>
      <Plot w={W} h={H} line={smooth} color={SIGNAL.cv}>
        <svg
          viewBox={`0 0 ${W} ${H}`}
          style={{ positionType: "absolute", width: "100%", height: "100%" }}
        >
          <path
            d={stepped}
            fill="none"
            stroke={alpha(C.text, 0.25)}
            strokeWidth={1}
          />
        </svg>
      </Plot>
      <Row>
        <Knob
          name="TIME"
          value={time}
          onChange={(v) => set("time", v)}
          color={color}
          size={34}
          show={(v) => `${Math.round(slew(v) * 1000)}ms`}
        />
      </Row>
    </Col>
  );
}
