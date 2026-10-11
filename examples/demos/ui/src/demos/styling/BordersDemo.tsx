import { useState } from "react";
import { InlineCode, Paragraph } from "@/components/typography";
import { Box, ControlColumn, DemoRow, Example, Slider } from "@/components";
import { Code } from "@/components/docs";
import { useDemoPage, type ExplanationData } from "@/explanationStore";
import { Colors } from "@/theme";

const PAGE: ExplanationData = {
  title: "Borders",
  info: (
    <>
      <Paragraph>
        <InlineCode>border</InlineCode> sets the edge width — and it
        participates in layout, so a thicker border takes real space.{" "}
        <InlineCode>borderColor</InlineCode> paints it, and{" "}
        <InlineCode>borderRadius</InlineCode> rounds the corners — into circular
        arcs, or elliptical ones with a slash.
      </Paragraph>
      <Code lang="tsx">{`<node
  style={{
    border: 2,
    borderColor: "#5cd9ff",
    borderRadius: { top: 0, right: 10, bottom: 20, left: 60 },
  }}
/>`}</Code>
      <Paragraph>
        Each of the three accepts a single value or a per-side{" "}
        <InlineCode>{"{ top, right, bottom, left }"}</InlineCode> object;{" "}
        <InlineCode>border</InlineCode> and{" "}
        <InlineCode>borderRadius</InlineCode> also take an axis pair{" "}
        <InlineCode>{"{ horizontal, vertical }"}</InlineCode> (
        <InlineCode>borderColor</InlineCode> is per-side only).{" "}
        <InlineCode>outline</InlineCode> draws an extra ring outside the box —{" "}
        <InlineCode>width</InlineCode>, <InlineCode>offset</InlineCode>,{" "}
        <InlineCode>color</InlineCode> — and is ignored by layout.
      </Paragraph>
    </>
  ),
};

export function BordersDemo() {
  useDemoPage(PAGE);
  return (
    <>
      <DemoRow>
        <BorderRadiusDemo />
        <EllipticalDemo />
      </DemoRow>
      <DemoRow>
        <BorderWidthDemo />
        <PerSideDemo />
      </DemoRow>
      <DemoRow>
        <OutlineDemo />
      </DemoRow>
    </>
  );
}

function BorderRadiusDemo() {
  return (
    <Example
      title="Rounded corners"
      info={
        <>
          <Paragraph>
            <InlineCode>borderRadius</InlineCode> rounds the corners. Drag the
            slider from a square corner all the way to a pill.
          </Paragraph>
          <Code lang="tsx">{`<node style={{ borderRadius: 16 }} />`}</Code>
        </>
      }
      demo={BorderRadiusCard}
    />
  );
}

function BorderRadiusCard() {
  const [r, setR] = useState(16);
  return (
    <ControlColumn>
      <Box style={{ borderRadius: r }} />
      <Slider value={r} min={0} max={36} onChange={setR} name="borderRadius" />
    </ControlColumn>
  );
}

function EllipticalDemo() {
  return (
    <Example
      title="Elliptical corners"
      info={
        <>
          <Paragraph>
            A <InlineCode>/</InlineCode> in the string splits it into horizontal
            and vertical radii, as in CSS. With a slash a percentage follows its
            own axis, so <InlineCode>"50% / 50%"</InlineCode> is an ellipse,
            while a plain <InlineCode>"50%"</InlineCode> stays a pill.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{
    width: 144,
    height: 72,
    borderRadius: "50% / 50%",
  }}
/>`}</Code>
        </>
      }
      demo={EllipticalCard}
    />
  );
}

function EllipticalCard() {
  const [x, setX] = useState(50);
  const [y, setY] = useState(50);
  return (
    <ControlColumn>
      <Box
        style={{
          width: 144,
          border: 3,
          borderColor: Colors.amber,
          borderRadius: `${x}% / ${y}%`,
        }}
      />
      <Slider value={x} min={0} max={50} onChange={setX} name="horizontal %" />
      <Slider value={y} min={0} max={50} onChange={setY} name="vertical %" />
    </ControlColumn>
  );
}

function BorderWidthDemo() {
  return (
    <Example
      title="Border width and color"
      info={
        <>
          <Paragraph>
            <InlineCode>border</InlineCode> adds an edge of the given width,
            painted by <InlineCode>borderColor</InlineCode>. The width is part
            of layout, so growing it shrinks the content box.
          </Paragraph>
          <Code lang="tsx">{`<node style={{ border: 2, borderColor: "#5cd9ff" }} />`}</Code>
        </>
      }
      demo={BorderWidthCard}
    />
  );
}

function BorderWidthCard() {
  const [w, setW] = useState(2);
  return (
    <ControlColumn>
      <Box
        style={{
          backgroundColor: Colors.card,
          border: w,
          borderColor: Colors.cyan,
        }}
      />
      <Slider value={w} min={0} max={12} onChange={setW} name="border" />
    </ControlColumn>
  );
}

function PerSideDemo() {
  return (
    <Example
      title="Per-side values"
      info={
        <>
          <Paragraph>
            Every border attribute also takes a per-side{" "}
            <InlineCode>{"{ top, right, bottom, left }"}</InlineCode> object, so
            each edge can have its own width, color, and corner radius. On{" "}
            <InlineCode>borderRadius</InlineCode> those keys name the four
            corners, so the axis pair{" "}
            <InlineCode>{"{ horizontal }"}</InlineCode> is the top-right and
            bottom-left pair.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{
    borderRadius: { top: 0, right: 10, bottom: 20, left: 60 },
    border: { top: 3, right: 6, bottom: 9, left: 12 },
    borderColor: {
      top: "#5cd9ff",
      right: "#ffc857",
      bottom: "#ff6b8b",
      left: "#5ee6a8",
    },
  }}
/>`}</Code>
        </>
      }
      demo={PerSideCard}
    />
  );
}

function PerSideCard() {
  return (
    <ControlColumn>
      <Box
        style={{
          backgroundColor: Colors.card,
          borderRadius: {
            top: 0,
            right: 10,
            bottom: 20,
            left: 60,
          },
          border: {
            top: 3,
            right: 6,
            bottom: 9,
            left: 12,
          },
          borderColor: {
            top: Colors.cyan,
            right: Colors.amber,
            bottom: Colors.rose,
            left: Colors.mint,
          },
        }}
      />
    </ControlColumn>
  );
}

function OutlineDemo() {
  return (
    <Example
      title="Outlines"
      info={
        <>
          <Paragraph>
            <InlineCode>outline</InlineCode> draws a ring outside the box — it
            is ignored by layout, so changing its width or offset never moves
            anything. Drag the offset to float the ring away from the edge.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{ outline: { width: 3, offset: 4, color: "#ffc857" } }}
/>`}</Code>
        </>
      }
      demo={OutlineCard}
    />
  );
}

function OutlineCard() {
  const [offset, setOffset] = useState(4);
  return (
    <ControlColumn>
      <Box style={{ outline: { width: 3, offset, color: Colors.amber } }} />
      <Slider
        value={offset}
        min={0}
        max={16}
        onChange={setOffset}
        name="outline offset"
      />
    </ControlColumn>
  );
}
