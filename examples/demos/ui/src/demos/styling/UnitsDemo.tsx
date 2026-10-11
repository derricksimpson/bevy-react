import { useEffect, useState } from "react";
import {
  Caption,
  InlineCode,
  ListItem,
  Paragraph,
  List,
} from "@/components/typography";
import { BevyStyle } from "bevy-react/jsx";
import {
  Box,
  ControlColumn,
  DemoRow,
  Example,
  Figure,
  Row,
  Slider,
  Stage,
  stage,
} from "@/components";
import { Code, CodeTabs } from "@/components/docs";
import { bevy } from "@/bevy";
import { useDemoPage, type ExplanationData } from "@/explanationStore";
import { Colors } from "@/theme";

const PAGE: ExplanationData = {
  title: "Units",
  info: (
    <>
      <Paragraph>
        Style values accept units. A bare number picks a sensible default per
        field — px for lengths and font sizes, degrees for angles, milliseconds
        for durations. Strings carry an explicit unit:
      </Paragraph>
      <Code lang="tsx">{`width: 80            // px
width: "50%"         // of the parent
width: "10vw"        // of the viewport
width: "12rem"       // of the root font size
padding: "0.5em"     // of the font size
fontSize: "1.5rem"   // of Bevy's RemSize
rotate: "0.25turn"   // deg / rad / turn / grad
duration: "0.3s"     // ms / s`}</Code>
      <List>
        <ListItem>Lengths: px, %, vw/vh/vmin/vmax, rem, em, auto.</ListItem>
        <ListItem>fontSize takes px, viewport units, rem and em.</ListItem>
        <ListItem>Angles: deg (bare-number default), rad, turn, grad.</ListItem>
        <ListItem>Time: ms (bare-number default), s.</ListItem>
      </List>
    </>
  ),
};

export function UnitsDemo() {
  useDemoPage(PAGE);
  return (
    <>
      <DemoRow>
        <LengthDemo />
        <FontSizeDemo />
      </DemoRow>
      <DemoRow>
        <AngleDemo />
        <TimeDemo />
      </DemoRow>
      <DemoRow>
        <RootSizeDemo />
      </DemoRow>
    </>
  );
}

const LENGTHS = ["80px", "50%", "10vw", "6rem"];

function LengthDemo() {
  return (
    <Example
      title="Lengths"
      info={
        <>
          <Paragraph>
            A bare number is px; strings carry a unit.{" "}
            <InlineCode>%</InlineCode> is relative to the parent,{" "}
            <InlineCode>vw/vh/vmin/vmax</InlineCode> to the viewport,{" "}
            <InlineCode>rem</InlineCode> to the root font size, plus{" "}
            <InlineCode>auto</InlineCode>. Resize the window and the vw bar
            follows it.
          </Paragraph>
          <Code lang="tsx">{`<node style={{ width: "80px" }} />
<node style={{ width: "50%" }} />
<node style={{ width: "10vw" }} />
<node style={{ width: "6rem" }} />`}</Code>
        </>
      }
      demo={LengthCard}
    />
  );
}

function LengthCard() {
  return (
    <node style={{ flexDirection: "column", gap: 10, width: "100%" }}>
      {LENGTHS.map((w) => (
        <node key={w} style={{ flexDirection: "column", gap: 4 }}>
          <Caption mono>{w}</Caption>
          <node
            style={{
              width: w,
              height: 28,
              borderRadius: 6,
              backgroundColor: Colors.cyan,
            }}
          />
        </node>
      ))}
    </node>
  );
}

const FONT_SIZES = ["14px", "1.5rem", "1.2em", "2vw"];

function FontSizeDemo() {
  return (
    <Example
      title="Font sizes"
      info={
        <>
          <Paragraph>
            Font size takes px, viewport units, or <InlineCode>rem</InlineCode>{" "}
            — relative to Bevy's <InlineCode>RemSize</InlineCode> resource
            (default 20px), the knob for app-wide text scaling.{" "}
            <InlineCode>em</InlineCode> is relative to the size the text
            inherits: the root size here, the enclosing text's on a nested{" "}
            <InlineCode>{"<text>"}</InlineCode>.
          </Paragraph>
          <Code lang="tsx">{`<text style={{ fontSize: "14px" }} />
<text style={{ fontSize: "1.5rem" }} />
<text style={{ fontSize: "1.2em" }} />
<text style={{ fontSize: "2vw" }} />`}</Code>
        </>
      }
      demo={FontSizeCard}
    />
  );
}

function FontSizeCard() {
  return (
    <node style={{ flexDirection: "column", gap: 12 }}>
      {FONT_SIZES.map((size) => (
        <node
          key={size}
          style={{ flexDirection: "row", alignItems: "center", gap: 14 }}
        >
          <node style={{ width: 90 }}>
            <Caption mono>{size}</Caption>
          </node>
          <text
            style={{
              fontSize: size,
              color: Colors.text,
              fontWeight: "bold",
            }}
          >
            Aa Bb Cc
          </text>
        </node>
      ))}
    </node>
  );
}

function RootSizeDemo() {
  return (
    <Example
      title="Root font size"
      info={
        <>
          <Paragraph>
            <InlineCode>rem</InlineCode> is a multiple of Bevy's{" "}
            <InlineCode>RemSize</InlineCode> resource.{" "}
            <InlineCode>em</InlineCode> is a multiple of the element's own font
            size: its <InlineCode>fontSize</InlineCode> on a text, the root size
            on a node. Drag the slider: the bar, the rem text and the node chip
            follow, the text chip keeps the padding of its own 28px. An{" "}
            <InlineCode>em</InlineCode> font size on a nested text is a multiple
            of the text around it.
          </Paragraph>
          <CodeTabs
            tsx={`<node style={{ width: "8rem" }} />
<text style={{ fontSize: "1rem" }} />

const chip = {
  padding: "0.5em",
  borderRadius: "0.25em",
};
const big = { ...chip, fontSize: 28 };

// em = the text's own 28px
<text style={big}>text</text>
// em = the root font size
<node style={chip}>…</node>

// half of the enclosing text
<text style={{ fontSize: 28 }}>
  Big
  <text style={{ fontSize: "0.5em" }}>
    half
  </text>
</text>`}
            rust={`#[react_message(
    name = "units.setRemSize"
)]
pub struct SetRemSize(pub f32);

fn set_rem_size(
    on: On<SetRemSize>,
    mut rem: ResMut<RemSize>,
) {
    rem.0 = on.event().0;
}

app.add_react_handler(set_rem_size);`}
          />
        </>
      }
      demo={RootSizeCard}
    />
  );
}

const DEFAULT_REM = 20;

function RootSizeCard() {
  const [rem, setRem] = useState(DEFAULT_REM);
  // The root size belongs to the whole app: give it back with the page.
  useEffect(() => () => bevy.units.setRemSize(DEFAULT_REM), []);
  const change = (value: number) => {
    const px = Math.round(value);
    setRem(px);
    bevy.units.setRemSize(px);
  };

  return (
    <ControlColumn>
      <node style={{ flexDirection: "column", gap: 10, width: "100%" }}>
        <node style={{ ...remBar, width: "8rem" }} />
        <text style={{ fontSize: "1rem", color: Colors.text }}>1rem text</text>
        <Row>
          <text style={{ ...chip, fontSize: 28 }}>text</text>
          <node style={chip}>
            <text style={{ fontSize: 28, color: Colors.ink }}>node</text>
          </node>
        </Row>
        <text style={{ fontSize: 28, color: Colors.text }}>
          Big{" "}
          <text style={{ fontSize: "0.5em", color: Colors.cyan }}>half</text>
        </text>
      </node>
      <Slider
        value={rem}
        min={12}
        max={32}
        onChange={change}
        name="RemSize"
        unit="px"
      />
    </ControlColumn>
  );
}

const remBar: BevyStyle = {
  height: 28,
  borderRadius: 6,
  backgroundColor: Colors.cyan,
};

// The same em values on a text (its own font size) and on a node (the root
// size).
const chip: BevyStyle = {
  padding: "0.5em",
  borderRadius: "0.25em",
  backgroundColor: Colors.mint,
  color: Colors.ink,
};

const ANGLES = ["45deg", "0.785rad", "0.125turn", "50grad"];

function AngleDemo() {
  return (
    <Example
      title="Angles"
      info={
        <>
          <Paragraph>
            A bare number is degrees; strings carry{" "}
            <InlineCode>deg/rad/turn/grad</InlineCode>. These four boxes are the
            same 45° written four ways.
          </Paragraph>
          <Code lang="tsx">{`transform: { rotate: "45deg" }     // = 45
transform: { rotate: "0.785rad" }
transform: { rotate: "0.125turn" }
transform: { rotate: "50grad" }`}</Code>
        </>
      }
      demo={AngleCard}
    />
  );
}

function AngleCard() {
  return (
    <Row style={{ gap: 15 }}>
      {ANGLES.map((angle) => (
        <Figure key={angle} style={{ gap: 10 }} mono caption={angle}>
          <Stage>
            <Box
              style={{
                width: 40,
                height: 40,
                backgroundColor: Colors.violet,
                transform: { rotate: angle },
              }}
            />
          </Stage>
        </Figure>
      ))}
    </Row>
  );
}

function TimeDemo() {
  return (
    <Example
      title="Durations"
      info={
        <>
          <Paragraph>
            A bare number is milliseconds; strings carry{" "}
            <InlineCode>ms/s</InlineCode>. Both boxes ease identically — click
            either to toggle.
          </Paragraph>
          <Code lang="tsx">{`transition: { transform: { duration: "300ms" } }
transition: { transform: { duration: "0.3s" } } // the same`}</Code>
        </>
      }
      demo={TimeCard}
    />
  );
}

function TimeCard() {
  const [on, setOn] = useState(false);

  return (
    <Row style={{ gap: 20 }}>
      <TimeBox
        label="300ms"
        duration="300ms"
        on={on}
        onToggle={() => setOn((v) => !v)}
      />
      <TimeBox
        label="0.3s"
        duration="0.3s"
        on={on}
        onToggle={() => setOn((v) => !v)}
      />
    </Row>
  );
}

type TimeBoxProps = {
  label: string;
  duration: string;
  on: boolean;
  onToggle: () => void;
};

function TimeBox({ label, duration, on, onToggle }: TimeBoxProps) {
  return (
    <node style={{ flexDirection: "column", alignItems: "center", gap: 10 }}>
      <button onClick={onToggle} style={timeTrack}>
        <Box
          style={{
            width: 40,
            height: 40,
            backgroundColor: Colors.mint,
            transform: { translateX: on ? 70 : 0 },
            transition: { transform: { duration, easing: "easeOut" } },
          }}
        />
      </button>
      <Caption mono>{label}</Caption>
    </node>
  );
}

// The plate the box slides along: Stage's chrome on the clickable button.
const timeTrack: BevyStyle = {
  ...stage,
  width: 130,
  height: 64,
  justifyContent: "start",
  alignItems: "center",
};
