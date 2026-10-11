import { useState } from "react";
import { BoxLabel, InlineCode, Paragraph } from "@/components/typography";
import { BevyStyle } from "bevy-react/jsx";
import {
  Button,
  ControlColumn,
  DemoRow,
  Example,
  Radio,
  RadioOption,
  Slider,
  Stage,
  stage,
} from "@/components";
import { Code } from "@/components/docs";
import { Colors, FontSizes } from "@/theme";
import { useDemoPage, type ExplanationData } from "@/explanationStore";

const PAGE: ExplanationData = {
  title: "Positioning",
  info: (
    <>
      <Paragraph>
        <InlineCode>positionType</InlineCode> decides whether a node takes part
        in its parent's flex or grid layout; <InlineCode>left</InlineCode>,{" "}
        <InlineCode>right</InlineCode>, <InlineCode>top</InlineCode> and{" "}
        <InlineCode>bottom</InlineCode> offset it.
      </Paragraph>
      <Code lang="tsx">{`<node
  style={{
    positionType: "absolute",
    top: 8,
    right: 8,
  }}
/>`}</Code>
      <Paragraph>
        A <InlineCode>"relative"</InlineCode> node (the default) is nudged from
        its laid-out spot; an <InlineCode>"absolute"</InlineCode> node leaves
        the flow and is placed against its direct parent's padding box; a{" "}
        <InlineCode>"fixed"</InlineCode> node is placed against the window, and
        no ancestor scrolls or clips it.
      </Paragraph>
    </>
  ),
};

export function PositioningDemo() {
  useDemoPage(PAGE);
  return (
    <>
      <DemoRow>
        <OutOfFlowDemo />
        <RelativeDemo />
      </DemoRow>
      <DemoRow>
        <CornerDemo />
        <StretchDemo />
      </DemoRow>
      <DemoRow>
        <FixedDemo />
      </DemoRow>
    </>
  );
}

type Position = "relative" | "absolute";

const POSITION_OPTIONS: RadioOption<Position>[] = [
  { label: "relative", value: "relative" },
  { label: "absolute", value: "absolute" },
];

function OutOfFlowDemo() {
  return (
    <Example
      title="Out of the flow"
      info={
        <>
          <Paragraph>
            An absolute node takes up no space: its siblings close the gap as if
            it weren't there, and the insets place it against the parent.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{
    positionType: "absolute",
    right: 0,
    bottom: 0,
  }}
/>`}</Code>
        </>
      }
      demo={OutOfFlowCard}
    />
  );
}

function OutOfFlowCard() {
  const [position, setPosition] = useState<Position>("absolute");
  return (
    <ControlColumn>
      <Stage style={row}>
        <Cell label="1" color={Colors.cyan} />
        <node
          style={{
            ...cell,
            positionType: position,
            right: 0,
            bottom: 0,
            backgroundColor: Colors.rose,
          }}
        >
          <BoxLabel>2</BoxLabel>
        </node>
        <Cell label="3" color={Colors.amber} />
      </Stage>
      <Radio
        options={POSITION_OPTIONS}
        value={position}
        onChange={setPosition}
      />
    </ControlColumn>
  );
}

function RelativeDemo() {
  return (
    <Example
      title="Relative offsets"
      info={
        <>
          <Paragraph>
            A relative node is laid out in the flow, then moved by its insets.
            Siblings keep their places as if it hadn't moved.
          </Paragraph>
          <Code lang="tsx">{`<node style={{ left: 20, top: -10 }} />`}</Code>
        </>
      }
      demo={RelativeCard}
    />
  );
}

function RelativeCard() {
  const [left, setLeft] = useState(20);
  const [top, setTop] = useState(-10);
  return (
    <ControlColumn>
      <Stage style={row}>
        <Cell label="1" color={Colors.cyan} />
        <node
          style={{
            ...cell,
            left: Math.round(left),
            top: Math.round(top),
            backgroundColor: Colors.rose,
          }}
        >
          <BoxLabel>2</BoxLabel>
        </node>
        <Cell label="3" color={Colors.amber} />
      </Stage>
      <Slider
        value={left}
        min={-40}
        max={40}
        onChange={setLeft}
        name="left"
        unit="px"
      />
      <Slider
        value={top}
        min={-30}
        max={30}
        onChange={setTop}
        name="top"
        unit="px"
      />
    </ControlColumn>
  );
}

type Corner = "topLeft" | "topRight" | "bottomLeft" | "bottomRight";

const CORNER_OPTIONS: RadioOption<Corner>[] = [
  { label: "↖", value: "topLeft" },
  { label: "↗", value: "topRight" },
  { label: "↙", value: "bottomLeft" },
  { label: "↘", value: "bottomRight" },
];

function CornerDemo() {
  return (
    <Example
      title="Corner badges"
      info={
        <>
          <Paragraph>
            One inset per axis pins an absolute node to a corner. The insets are
            measured from the parent's padding box, inside its border.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{
    positionType: "absolute",
    top: 8,
    right: 8,
  }}
/>`}</Code>
        </>
      }
      demo={CornerCard}
    />
  );
}

function CornerCard() {
  const [corner, setCorner] = useState<Corner>("topRight");
  const [inset, setInset] = useState(8);
  const n = Math.round(inset);
  const vertical = corner.startsWith("top") ? { top: n } : { bottom: n };
  const horizontal = corner.endsWith("Left") ? { left: n } : { right: n };
  return (
    <ControlColumn>
      <Stage style={box}>
        <node style={{ ...badge, ...vertical, ...horizontal }} />
      </Stage>
      <Radio options={CORNER_OPTIONS} value={corner} onChange={setCorner} />
      <Slider
        value={inset}
        min={0}
        max={40}
        onChange={setInset}
        name="inset"
        unit="px"
      />
    </ControlColumn>
  );
}

function StretchDemo() {
  return (
    <Example
      title="Stretching overlays"
      info={
        <>
          <Paragraph>
            Insets on opposite sides with no size on that axis stretch the node
            between them; all four at <InlineCode>0</InlineCode> cover the
            parent.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{
    positionType: "absolute",
    left: 0,
    right: 0,
    top: 0,
    bottom: 0,
  }}
/>`}</Code>
        </>
      }
      demo={StretchCard}
    />
  );
}

function StretchCard() {
  const [inset, setInset] = useState(16);
  const n = Math.round(inset);
  return (
    <ControlColumn>
      <Stage style={box}>
        <node
          style={{
            positionType: "absolute",
            left: n,
            right: n,
            top: n,
            bottom: n,
            borderRadius: 8,
            backgroundColor: Colors.violet,
          }}
        />
      </Stage>
      <Slider
        value={inset}
        min={0}
        max={50}
        onChange={setInset}
        name="inset"
        unit="px"
      />
    </ControlColumn>
  );
}

type Pinned = "absolute" | "fixed";

const PINNED_OPTIONS: RadioOption<Pinned>[] = [
  { label: "absolute", value: "absolute" },
  { label: "fixed", value: "fixed" },
];

function FixedDemo() {
  return (
    <Example
      title="Fixed to the window"
      info={
        <>
          <Paragraph>
            A fixed node is placed against the window, not its parent. It takes
            no space and no ancestor scrolls or clips it: the badge leaves the
            list for the window's corner, and stays there while the list and the
            page scroll.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{
    positionType: "fixed",
    right: 24,
    bottom: 24,
  }}
/>`}</Code>
          <Paragraph>
            It is still a child of the list: it unmounts with it, its events
            bubble through it, and it paints in tree order (
            <InlineCode>globalZIndex</InlineCode> lifts it above everything). An
            ancestor's <InlineCode>opacity</InlineCode>,{" "}
            <InlineCode>filter</InlineCode> or 3D transform doesn't reach it.
          </Paragraph>
        </>
      }
      demo={FixedCard}
    />
  );
}

// Starts absolute: the page shouldn't open with a badge over the window, and
// the Details modal mounts a second instance of this card.
function FixedCard() {
  const [position, setPosition] = useState<Pinned>("absolute");
  // Which end the button last sent the list to. Not mirrored from
  // `onScroll`: feeding the eased offset back as `scrollTop` would retarget
  // the glide to where it already is.
  const [atEnd, setAtEnd] = useState(false);
  return (
    <ControlColumn>
      {/* `scrollTop` is a node prop Stage doesn't forward. */}
      <node style={{ ...stage, ...list }} scrollTop={atEnd ? 10_000 : 0}>
        {ROWS.map((row) => (
          <node key={row} style={listRow}>
            <text style={listLabel}>{row}</text>
          </node>
        ))}
        <node
          style={{
            ...pill,
            positionType: position,
            ...(position === "fixed" ? windowCorner : listCorner),
          }}
        >
          <BoxLabel>{position}</BoxLabel>
        </node>
      </node>
      <node style={controls}>
        <Radio
          options={PINNED_OPTIONS}
          value={position}
          onChange={setPosition}
        />
        <Button onClick={() => setAtEnd(!atEnd)}>Scroll the list</Button>
      </node>
    </ControlColumn>
  );
}

function Cell({ label, color }: { label: string; color: string }) {
  return (
    <node style={{ ...cell, backgroundColor: color }}>
      <BoxLabel>{label}</BoxLabel>
    </node>
  );
}

const row: BevyStyle = {
  width: 240,
  height: 100,
  gap: 8,
  alignItems: "center",
};

const cell: BevyStyle = {
  width: 48,
  height: 48,
  borderRadius: 8,
  justifyContent: "center",
  alignItems: "center",
};

const box: BevyStyle = {
  width: 240,
  height: 140,
};

const badge: BevyStyle = {
  positionType: "absolute",
  width: 20,
  height: 20,
  borderRadius: 10,
  backgroundColor: Colors.rose,
};

const ROWS = Array.from({ length: 8 }, (_, i) => `Row ${i + 1}`);

const list: BevyStyle = {
  flexDirection: "column",
  gap: 6,
  width: 240,
  height: 140,
  overflowY: "scroll",
  scrollbarWidth: 8,
  transition: { scroll: { duration: 300, easing: "easeOut" } },
};

const listRow: BevyStyle = {
  flexShrink: 0,
  padding: "8px 12px",
  borderRadius: 6,
  border: 1,
  borderColor: Colors.line,
  backgroundColor: Colors.raised,
};

const listLabel: BevyStyle = {
  color: Colors.textBody,
  fontSize: FontSizes.sm,
};

const pill: BevyStyle = {
  padding: { horizontal: 12, vertical: 6 },
  borderRadius: 14,
  backgroundColor: Colors.rose,
};

// In the list: scrolls with the rows and is clipped with them.
const listCorner: BevyStyle = { top: 12, right: 20 };

// On the window: nothing above it moves or cuts it.
const windowCorner: BevyStyle = { right: 24, bottom: 24 };

const controls: BevyStyle = {
  flexDirection: "row",
  alignItems: "center",
  gap: 12,
};
