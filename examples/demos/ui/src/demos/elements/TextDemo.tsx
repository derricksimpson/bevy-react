import { useState } from "react";
import { Bold, InlineCode, Paragraph } from "@/components/typography";
import type { BevyStyle } from "bevy-react/jsx";
import { Checkbox, DemoRow, Example, Radio, Slider, Stage } from "@/components";
import { Code, CodeTabs } from "@/components/docs";
import { Colors, FontSizes } from "@/theme";
import { useDemoPage, type ExplanationData } from "@/explanationStore";

const PAGE: ExplanationData = {
  title: "<text>",
  info: (
    <>
      <Paragraph>
        <InlineCode>{"<text>"}</InlineCode> renders a run of styled glyphs — it
        is the only way to put text on screen (there is no bare-string rendering
        outside it). Font size, weight, family, color, line height, letter
        spacing, and shadow are all regular style fields.
      </Paragraph>
      <Paragraph>
        Nesting one <InlineCode>{"<text>"}</InlineCode> inside another restyles
        an <Bold>inline span</Bold> of the parent's run — that is how you color
        or bold part of a sentence. Spans do not inherit the parent's font
        settings: restate <InlineCode>fontSize</InlineCode> /{" "}
        <InlineCode>fontFamily</InlineCode> on the span if they should match, or
        size it relative to the text around it with a font size in{" "}
        <InlineCode>em</InlineCode>.
      </Paragraph>
      <Code lang="tsx">{`<text style={{ fontSize: 18 }}>
  Nested <text style={{ color: "#5cd9ff", fontSize: 18 }}>spans</text> restyle
  part of a sentence.
</text>`}</Code>
      <Paragraph>
        Alignment-class fields (<InlineCode>textAlign</InlineCode>,{" "}
        <InlineCode>lineBreak</InlineCode>, <InlineCode>textShadow</InlineCode>)
        apply on the root <InlineCode>{"<text>"}</InlineCode> only. An{" "}
        <InlineCode>{"<image>"}</InlineCode> inside a{" "}
        <InlineCode>{"<text>"}</InlineCode> is an <Bold>inline image</Bold>: it
        flows with the glyphs. Click any example below for details and its code.
      </Paragraph>
    </>
  ),
};

const PARAGRAPH =
  "Line height, letter spacing, and a drop shadow give a block of text its rhythm and weight.";

type LineBreak = NonNullable<BevyStyle["lineBreak"]>;

const LINE_BREAKS: { label: string; value: LineBreak }[] = [
  { label: "wordBoundary", value: "wordBoundary" },
  { label: "anyCharacter", value: "anyCharacter" },
  { label: "wordOrCharacter", value: "wordOrCharacter" },
  { label: "noWrap", value: "noWrap" },
];

export function TextDemo() {
  useDemoPage(PAGE);
  return (
    <>
      <DemoRow>
        <FontSizeDemo />
        <FontFamilyDemo />
      </DemoRow>

      <DemoRow>
        <TypographyDemo />
        <LineBreakDemo />
      </DemoRow>

      <DemoRow>
        <InlineImageDemo />
      </DemoRow>
    </>
  );
}

function FontSizeDemo() {
  return (
    <Example
      title="Size and weight"
      info={
        <>
          <Paragraph>
            <InlineCode>fontSize</InlineCode> takes a number of logical pixels
            (or a unit string like <InlineCode>"2vw"</InlineCode>,{" "}
            <InlineCode>"1.5rem"</InlineCode> or, relative to the enclosing
            text, <InlineCode>"0.8em"</InlineCode>).{" "}
            <InlineCode>fontWeight</InlineCode> picks a named weight from{" "}
            <InlineCode>thin</InlineCode> to <InlineCode>black</InlineCode> —
            with a variable font, every step renders true.
          </Paragraph>
          <Code lang="tsx">{`<text style={{ fontSize: 28, fontWeight: "bold" }}>
  Big & bold
</text>`}</Code>
        </>
      }
      demo={FontSizeCard}
    />
  );
}

function FontSizeCard() {
  const [size, setSize] = useState(28);
  return (
    <node
      style={{
        flexDirection: "column",
        alignItems: "center",
        gap: 16,
        width: "100%",
      }}
    >
      <text style={{ ...sampleStyle, fontSize: size, fontWeight: "thin" }}>
        thin
      </text>
      <text style={{ ...sampleStyle, fontSize: size, fontWeight: "normal" }}>
        normal
      </text>
      <text style={{ ...sampleStyle, fontSize: size, fontWeight: "bold" }}>
        bold
      </text>

      <Slider
        value={size}
        min={10}
        max={48}
        onChange={setSize}
        name="fontSize"
      />
    </node>
  );
}

function FontFamilyDemo() {
  return (
    <Example
      title="Fonts and spans"
      info={
        <>
          <Paragraph>
            Fonts are registered <Bold>up front on the Rust side</Bold> and
            selected by name from React — <InlineCode>fontFamily</InlineCode>{" "}
            cannot load a file at runtime. A comma list is a fallback chain: a
            glyph the first font lacks comes from the next one. Inline spans (a
            nested <InlineCode>{"<text>"}</InlineCode>) recolor or re-weight
            part of one paragraph.
          </Paragraph>
          <CodeTabs
            tsx={`<text style={{ fontFamily: "DancingScript" }}>
  Styled with a custom font family
</text>

<text
  style={{
    fontFamily: "Space Grotesk, Inter",
  }}
>
  Saved ✓
</text>

<text style={{ fontSize: 18 }}>
  Nested texts color{" "}
  <text style={{ color: "#5cd9ff", fontSize: 18 }}>part</text> of a sentence.
</text>`}
            rust={`ReactUiPlugin::new(bundle)
    .default_font("fonts/Inter.ttf")
    .font("Inter", "fonts/Inter.ttf")
    .font("Space Grotesk", "fonts/Grotesk.ttf")
    .font("DancingScript", "fonts/DancingScript.ttf")`}
          />
        </>
      }
      demo={FontFamilyCard}
    />
  );
}

function FontFamilyCard() {
  return (
    <>
      <text
        style={{
          fontFamily: "DancingScript",
          fontSize: FontSizes.xxl,
          color: Colors.amber,
        }}
      >
        Styled with a custom font family
      </text>

      <node style={{ gap: 24 }}>
        {FALLBACKS.map((fontFamily) => (
          <node key={fontFamily} style={fallbackSample}>
            <text
              style={{ ...sampleStyle, fontSize: FontSizes.xl, fontFamily }}
            >
              Saved ✓
            </text>
            <InlineCode>{fontFamily}</InlineCode>
          </node>
        ))}
      </node>

      <text style={sampleStyle}>
        Nested texts color{" "}
        <text
          style={{ ...sampleStyle, color: Colors.cyan, fontWeight: "bold" }}
        >
          part
        </text>{" "}
        of a{" "}
        <text
          style={{ ...sampleStyle, color: Colors.rose, fontWeight: "bold" }}
        >
          sentence
        </text>
        .
      </text>
    </>
  );
}

function TypographyDemo() {
  return (
    <Example
      title="Typography"
      info={
        <>
          <Paragraph>
            <InlineCode>lineHeight</InlineCode> (a multiple of the font size, or
            px), <InlineCode>letterSpacing</InlineCode> (px), and{" "}
            <InlineCode>textShadow</InlineCode> tune how a block of text sits on
            the page. All three are plain style fields — toggle or animate them
            like any other.
          </Paragraph>
          <Code lang="tsx">{`<text
  style={{
    lineHeight: 1.8,
    letterSpacing: 2,
    textShadow: { color: "#000", offsetX: 2, offsetY: 2 },
  }}
>
  {paragraph}
</text>`}</Code>
        </>
      }
      demo={TypographyCard}
    />
  );
}

function TypographyCard() {
  const [lineHeight, setLineHeight] = useState(1.4);
  const [letterSpacing, setLetterSpacing] = useState(1.5);
  const [shadow, setShadow] = useState(true);

  return (
    <node style={{ flexDirection: "column", gap: 16, maxWidth: 380 }}>
      <text
        style={{
          fontSize: FontSizes.base,
          color: Colors.text,
          lineHeight,
          letterSpacing,
          textShadow: shadow
            ? { color: "#000000cc", offsetX: 2, offsetY: 2 }
            : undefined,
        }}
      >
        {PARAGRAPH}
      </text>
      <Slider
        value={lineHeight}
        min={1}
        max={2.5}
        onChange={setLineHeight}
        name="lineHeight"
      />
      <Slider
        value={letterSpacing}
        min={0}
        max={8}
        onChange={setLetterSpacing}
        name="letterSpacing"
        decimals={1}
        unit="px"
      />
      <Checkbox label="textShadow" enabled={shadow} onChange={setShadow} />
    </node>
  );
}

function LineBreakDemo() {
  return (
    <Example
      title="Line breaking"
      info={
        <>
          <Paragraph>
            <InlineCode>lineBreak</InlineCode> picks the wrapping mode when text
            overflows its width: break at word boundaries (default), at any
            character, prefer words but fall back to characters, or never wrap
            at all. Root <InlineCode>{"<text>"}</InlineCode> only.
          </Paragraph>
          <Code lang="tsx">{`<text style={{ width: 220, lineBreak: "anyCharacter" }}>
  Pneumonoultramicroscopicsilicovolcanoconiosis
</text>`}</Code>
        </>
      }
      demo={LineBreakCard}
    />
  );
}

function LineBreakCard() {
  const [mode, setMode] = useState<LineBreak>("wordBoundary");
  return (
    <node style={{ flexDirection: "column", alignItems: "center", gap: 16 }}>
      <Stage style={{ width: 220 }}>
        <text
          style={{
            fontSize: FontSizes.sm,
            color: Colors.textBody,
            lineBreak: mode,
          }}
        >
          Pneumonoultramicroscopicsilicovolcanoconiosis wraps differently per
          mode.
        </text>
      </Stage>
      <Radio wrap value={mode} options={LINE_BREAKS} onChange={setMode} />
    </node>
  );
}

function InlineImageDemo() {
  return (
    <Example
      title="Inline images"
      info={
        <>
          <Paragraph>
            An <InlineCode>{"<image>"}</InlineCode> written inside a{" "}
            <InlineCode>{"<text>"}</InlineCode> is an <Bold>inline image</Bold>:
            it sits on the baseline, wraps with the words around it, and belongs
            to the block for clicks and layer styles. Size it with{" "}
            <InlineCode>width</InlineCode> / <InlineCode>height</InlineCode> in
            px, or in <InlineCode>em</InlineCode> to follow the font size of the
            text around it; one side alone keeps the texture's aspect ratio. A
            taller image grows its line.
          </Paragraph>
          <Paragraph>
            It takes <InlineCode>src</InlineCode>, <InlineCode>tint</InlineCode>
            , <InlineCode>flipX</InlineCode> / <InlineCode>flipY</InlineCode>{" "}
            and <InlineCode>opacity</InlineCode>; without a{" "}
            <InlineCode>src</InlineCode> it is a solid{" "}
            <InlineCode>tint</InlineCode> box. Use a texture close to the drawn
            size: <InlineCode>imageRendering</InlineCode> does not reach inline
            images.
          </Paragraph>
          <Code lang="tsx">{`<text style={{ fontSize: 18 }}>
  Built with{" "}
  <image
    src="images/inline/bevy-logo.png"
    style={{ height: "1em" }}
  />{" "}
  Bevy
</text>`}</Code>
        </>
      }
      demo={InlineImageCard}
    />
  );
}

const TINTS: { label: string; value: string }[] = [
  { label: "none", value: "#ffffff" },
  { label: "amber", value: Colors.amber },
  { label: "rose", value: Colors.rose },
];

function InlineImageCard() {
  const [fontSize, setFontSize] = useState(20);
  const [height, setHeight] = useState(1.2);
  const [tint, setTint] = useState(TINTS[0].value);
  const icon: BevyStyle = { height: `${height}em` };
  return (
    <node style={{ flexDirection: "column", alignItems: "center", gap: 16 }}>
      <Stage style={{ width: 300 }}>
        <text style={{ fontSize, color: Colors.textBody }}>
          <image
            tint={Colors.cyan}
            style={{ width: "0.5em", height: "0.5em" }}
          />{" "}
          Built with{" "}
          <image src="images/inline/bevy-logo.png" tint={tint} style={icon} />{" "}
          Bevy and <image src="images/inline/react-logo.png" style={icon} />{" "}
          React. Icons sit on the baseline and wrap with the words.
        </text>
      </Stage>
      <Slider
        value={fontSize}
        min={12}
        max={28}
        onChange={setFontSize}
        name="fontSize"
      />
      <Slider
        value={height}
        min={0.5}
        max={2.5}
        onChange={setHeight}
        name="height"
        decimals={1}
        unit="em"
      />
      <Radio value={tint} options={TINTS} onChange={setTint} />
    </node>
  );
}

// The demo's sample text. Spans restate it — they don't inherit the run's
// size or color.
// Space Grotesk has no check mark: alone it draws its missing-glyph box, in
// a list the glyph comes from the next font that has one.
const FALLBACKS = ["Space Grotesk", "Space Grotesk, Inter"];

const fallbackSample: BevyStyle = {
  flexDirection: "column",
  alignItems: "center",
  gap: 6,
};

const sampleStyle: BevyStyle = {
  fontSize: FontSizes.lg,
  color: Colors.text,
};
