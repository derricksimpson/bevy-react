import { useRef } from "react";
import { C, FONT_MONO, alpha, r } from "../theme";
import { Knob, Led, Tabs, clamp01, stepLight, useLive } from "../widgets";
import { type BodyProps, Col, Row } from "./kit";

/** The modules that keep time: the clock and the sequencer. */

export function Clock({ node, set, color }: BodyProps) {
  const { bpm } = useLive();
  const { div, swing } = node.params;
  return (
    <Row gap={12}>
      <Col gap={10}>
        <Row gap={12}>
          <node
            style={{ flexDirection: "row", alignItems: "flexEnd", gap: r(5) }}
          >
            <text
              style={{ fontSize: r(26), fontFamily: FONT_MONO, color: C.text }}
            >
              {bpm}
            </text>
            <text
              style={{ fontSize: r(9), color: C.dim, margin: { bottom: r(5) } }}
            >
              BPM
            </text>
          </node>
          <Row gap={6}>
            {[0, 1, 2, 3].map((k) => (
              <Led
                key={k}
                size={9}
                color={k ? color : "#ffffff"}
                level={(j) => (j % 8 === k * 2 ? 1 : 0)}
              />
            ))}
          </Row>
        </Row>
        <Tabs
          options={["1/4", "1/8", "1/16"]}
          value={div}
          onChange={(i) => set("div", i)}
          color={color}
        />
      </Col>
      <Knob
        name="SWING"
        value={swing}
        onChange={(v) => set("swing", v)}
        color={color}
        size={30}
        // Where the late pulse lands between its pair, as drum machines say.
        show={(v) => `${Math.round(50 + (v * 100) / 6)}%`}
      />
    </Row>
  );
}

export function Sequencer({ node, set, color, steps }: BodyProps) {
  const mask = node.params.mask;
  return (
    <node
      style={{
        flexGrow: 1,
        flexDirection: "row",
        alignItems: "center",
        gap: r(6),
      }}
    >
      {Array.from({ length: 8 }, (_, i) => {
        const on = ((mask >> i) & 1) === 1;
        const here = (j: number) => (j % 8 === i ? 1 : 0);
        return (
          <node
            key={i}
            style={{
              flexGrow: 1,
              flexBasis: 0,
              flexDirection: "column",
              alignItems: "center",
              gap: r(7),
            }}
          >
            <Bar
              value={node.params[`p${i}`]}
              onChange={(v) => set(`p${i}`, v)}
              color={on ? color : C.faint}
              glow={stepLight(steps, (j) => (on ? here(j) : 0), 0)}
            />
            <node
              onClick={() => set("mask", mask ^ (1 << i))}
              style={{
                width: r(16),
                height: r(9),
                borderRadius: r(3),
                backgroundColor: on ? alpha(color, 0.35) : C.cardDeep,
                border: r(1),
                borderColor: on ? alpha(color, 0.7) : C.edge,
              }}
              hoverStyle={{ borderColor: C.text }}
            >
              <node
                style={{
                  flexGrow: 1,
                  borderRadius: r(2),
                  backgroundColor: on ? "#ffffff" : C.dim,
                  opacity: { animated: stepLight(steps, here, 0) },
                }}
              />
            </node>
          </node>
        );
      })}
    </node>
  );
}

/** A sequencer step: drag its height. */
function Bar({
  value,
  onChange,
  color,
  glow,
}: {
  value: number;
  onChange: (v: number) => void;
  color: string;
  glow: ReturnType<typeof stepLight>;
}) {
  const drag = useRef<{ y: number; v: number } | null>(null);
  return (
    <node
      onPointerDown={(e) => {
        drag.current = { y: e.clientY, v: value };
      }}
      onPointerMove={(e) => {
        const d = drag.current;
        if (d) onChange(clamp01(d.v + (d.y - e.clientY) / 90));
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
      style={{
        width: "100%",
        height: r(58),
        flexDirection: "column",
        justifyContent: "flexEnd",
        borderRadius: r(4),
        backgroundColor: C.cardDeep,
        border: r(1),
        borderColor: C.edge,
        cursor: "nsResize",
      }}
      hoverStyle={{ borderColor: C.edgeHi }}
    >
      <node
        style={{
          height: `${Math.max(6, value * 100)}%`,
          borderRadius: r(3),
          backgroundGradient: {
            type: "linear",
            angle: 0,
            stops: [{ color: alpha(color, 0.25) }, { color }],
          },
        }}
      />
      <node
        style={{
          positionType: "absolute",
          left: 0,
          right: 0,
          top: 0,
          bottom: 0,
          borderRadius: r(3),
          backgroundColor: alpha("#ffffff", 0.35),
          opacity: { animated: glow },
        }}
      />
    </node>
  );
}
