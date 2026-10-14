import { useRef } from "react";
import { interpolate } from "bevy-react";
import { C, SIGNAL, alpha, r } from "../theme";
import { Knob, Meter, STEPS, Tabs, clamp01, rand, stepLight } from "../widgets";
import { type BodyProps, Col, Row, pct } from "./kit";

/** The modules that move levels and space: mixer, delay, reverb, output. */

export function Mixer({ node, set, color }: BodyProps) {
  return (
    <Row gap={12}>
      {[0, 1, 2, 3].map((i) => (
        <node
          key={i}
          style={{ flexDirection: "column", alignItems: "center", gap: r(5) }}
        >
          <node
            style={{ flexDirection: "row", gap: r(6), alignItems: "center" }}
          >
            <Fader
              value={node.params[`l${i}`]}
              onChange={(v) => set(`l${i}`, v)}
              color={color}
            />
            <Meter
              seed={11 + i}
              gain={node.params[`l${i}`]}
              color={SIGNAL.audio}
              height={68}
              width={6}
            />
          </node>
          <text style={{ fontSize: r(9), color: C.dim }}>{`CH ${i + 1}`}</text>
        </node>
      ))}
    </Row>
  );
}

function Fader({
  value,
  onChange,
  color,
}: {
  value: number;
  onChange: (v: number) => void;
  color: string;
}) {
  const drag = useRef<{ y: number; v: number } | null>(null);
  return (
    <node
      onPointerDown={(e) => {
        drag.current = { y: e.clientY, v: value };
      }}
      onPointerMove={(e) => {
        const d = drag.current;
        if (d) onChange(clamp01(d.v + (d.y - e.clientY) / 100));
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
      style={{
        width: r(22),
        height: r(68),
        flexDirection: "column",
        alignItems: "center",
        cursor: "nsResize",
      }}
    >
      <node
        style={{
          width: r(4),
          height: "100%",
          borderRadius: r(2),
          backgroundColor: C.cardDeep,
        }}
      />
      <node
        style={{
          positionType: "absolute",
          bottom: 0,
          width: r(4),
          height: `${value * 100}%`,
          borderRadius: r(2),
          backgroundColor: color,
        }}
      />
      <node
        style={{
          positionType: "absolute",
          bottom: `${value * 100}%`,
          margin: { bottom: r(-5) },
          width: r(20),
          height: r(10),
          borderRadius: r(3),
          border: r(1),
          borderColor: C.edgeHi,
          backgroundGradient: {
            type: "linear",
            angle: 180,
            stops: [{ color: "#3a4360" }, { color: "#202638" }],
          },
        }}
        hoverStyle={{ borderColor: color }}
      />
    </node>
  );
}

/** A synced delay's note values (`NOTES` in `synth/effects.rs`). */
const NOTES = ["1/16", "1/8", "1/8.", "1/4", "1/4.", "1/2"];

export function Delay({ node, set, color, steps }: BodyProps) {
  const { time, feedback, mix, sync } = node.params;
  return (
    <Col>
      <node
        style={{
          height: r(38),
          flexDirection: "row",
          alignItems: "flexEnd",
          gap: r(5),
          padding: { left: r(6), right: r(6), bottom: r(5) },
          borderRadius: r(6),
          backgroundColor: C.cardDeep,
          border: r(1),
          borderColor: C.edge,
        }}
      >
        {Array.from({ length: 12 }, (_, k) => (
          <node
            key={k}
            style={{
              flexGrow: 1,
              height: `${Math.max(6, 90 * feedback ** (k * (0.4 + time)))}%`,
              borderRadius: r(2),
              backgroundColor: k ? SIGNAL.audio : "#ffffff",
              opacity: {
                animated: stepLight(
                  steps,
                  (j) => ((j - k + STEPS) % 4 === 0 ? 1 : 0),
                  0.18,
                ),
              },
            }}
          />
        ))}
      </node>
      <Tabs
        options={["FREE", "SYNC"]}
        value={sync}
        onChange={(i) => set("sync", i)}
        color={color}
      />
      <Row gap={14}>
        <Knob
          name="TIME"
          value={time}
          onChange={(v) => set("time", v)}
          color={color}
          size={32}
          show={(v) =>
            sync ? NOTES[Math.round(v * 5)] : `${Math.round(40 + v * 960)}ms`
          }
        />
        <Knob
          name="FDBK"
          value={feedback}
          onChange={(v) => set("feedback", v)}
          color={color}
          size={32}
          show={pct}
        />
        <Knob
          name="MIX"
          value={mix}
          onChange={(v) => set("mix", v)}
          color={color}
          size={32}
          show={pct}
        />
      </Row>
    </Col>
  );
}

/** Reflections per side in the reverb's picture. */
const TAIL = 20;

export function Reverb({ node, set, color, steps }: BodyProps) {
  const { size, damp, mix } = node.params;
  // On every beat a wash runs out across the reflections, left above the
  // line and right below: longer the bigger the plate, dimmer the damper.
  const wash = (k: number) => {
    const input: number[] = [];
    const output: number[] = [];
    const late = (k / TAIL) * 1.6;
    const peak = Math.exp(-k / (TAIL * (0.12 + size * 0.7))) * (1 - damp * 0.5);
    for (let j = 0; j < STEPS; j += 2) {
      input.push(j + late, j + late + 0.04, j + late + 0.35);
      output.push(0.15, 0.15 + 0.85 * peak, 0.15);
    }
    return interpolate(steps, input, output);
  };
  const side = (top: boolean) => (
    <node
      style={{
        height: "50%",
        flexDirection: "row",
        alignItems: top ? "flexEnd" : "flexStart",
        gap: r(3),
      }}
    >
      {Array.from({ length: TAIL }, (_, k) => (
        <node
          key={k}
          style={{
            flexGrow: 1,
            height: `${Math.max(8, 95 * Math.exp(-k / (TAIL * (0.12 + size * 0.7))) * (0.55 + 0.45 * rand(top ? 31 : 37, k)))}%`,
            borderRadius: r(1.5),
            backgroundColor: alpha(SIGNAL.audio, 1 - damp * 0.45),
            opacity: { animated: wash(k) },
          }}
        />
      ))}
    </node>
  );
  return (
    <Col>
      <node
        style={{
          height: r(40),
          flexDirection: "column",
          padding: { horizontal: r(6), vertical: r(4) },
          borderRadius: r(6),
          backgroundColor: C.cardDeep,
          border: r(1),
          borderColor: C.edge,
        }}
      >
        {side(true)}
        {side(false)}
      </node>
      <Row gap={14}>
        <Knob
          name="SIZE"
          value={size}
          onChange={(v) => set("size", v)}
          color={color}
          size={32}
          show={pct}
        />
        <Knob
          name="DAMP"
          value={damp}
          onChange={(v) => set("damp", v)}
          color={color}
          size={32}
          show={pct}
        />
        <Knob
          name="MIX"
          value={mix}
          onChange={(v) => set("mix", v)}
          color={color}
          size={32}
          show={pct}
        />
      </Row>
    </Col>
  );
}

export function Output({ node, set, color, steps }: BodyProps) {
  const { volume } = node.params;
  // The cone kicks on every beat.
  const input: number[] = [];
  const output: number[] = [];
  for (let j = 0; j < STEPS; j += 2) {
    input.push(j, j + 0.05, j + 1.6);
    output.push(1, 1 + 0.12 * volume, 1);
  }
  const kick = interpolate(steps, input, output);
  return (
    <Row gap={14}>
      <node
        style={{
          width: r(68),
          height: r(68),
          borderRadius: "50%",
          alignItems: "center",
          justifyContent: "center",
          border: r(2),
          borderColor: C.edgeHi,
          backgroundGradient: {
            type: "radial",
            stops: [{ color: "#1e2434" }, { color: "#0b0e15" }],
          },
        }}
      >
        <node
          style={{
            width: r(46),
            height: r(46),
            borderRadius: "50%",
            alignItems: "center",
            justifyContent: "center",
            backgroundGradient: {
              type: "radial",
              position: "topLeft",
              stops: [{ color: "#4a5374" }, { color: "#161b28" }],
            },
            transform: { scale: { animated: kick } },
          }}
        >
          <node
            style={{
              width: r(14),
              height: r(14),
              borderRadius: "50%",
              backgroundColor: alpha(color, 0.8),
            }}
          />
        </node>
      </node>
      <Row gap={4}>
        <Meter seed={21} gain={volume} color={SIGNAL.audio} height={68} />
        <Meter
          seed={22}
          gain={volume * 0.92}
          color={SIGNAL.audio}
          height={68}
        />
      </Row>
      <Knob
        name="VOLUME"
        value={volume}
        onChange={(v) => set("volume", v)}
        color={color}
        show={pct}
      />
    </Row>
  );
}
