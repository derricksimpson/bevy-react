import { useState } from "react";
import { Caption, InlineCode, Paragraph } from "@/components/typography";
import { BevyStyle } from "bevy-react/jsx";
import { Button, Example, Stage } from "@/components";
import { Code } from "@/components/docs";

// The auto-height counterpart of the `size` accordion: the box's real height
// snaps between 1px and `auto`, the `layout` channel eases the box it SHOWS,
// and the `overflow` clip follows that eased box — so the panel is revealed
// without a fixed height and without a relayout per frame.

export function ClippedRevealDemo() {
  return (
    <Example
      title="Clipped reveals"
      info={
        <>
          <Paragraph>
            An <InlineCode>overflow</InlineCode> clip follows the box a{" "}
            <InlineCode>layout</InlineCode> transition shows, so a clipping
            wrapper reveals content of any height: the height is{" "}
            <InlineCode>auto</InlineCode>, and no frame relayouts.
          </Paragraph>
          <Code lang="tsx">{`<node
  style={{
    height: open ? "auto" : 1,
    overflowY: "hidden",
    flexDirection: "column",
    justifyContent: "center",
    transition: {
      layout: { duration: 300 },
    },
  }}
>
  <node style={{ flexShrink: 0 }}>
    {details}
  </node>
</node>`}</Code>
          <Paragraph>
            Children ride the box's center, so the content is centered in it:
            pinned to the top it would jump by half the height when the box
            collapses. The box collapses to <InlineCode>1</InlineCode>, because
            a box going to zero snaps; the panel fades out with it, or one pixel
            of its middle would stay in view. And only nodes with the channel
            ease — the footer has a <InlineCode>layout</InlineCode> entry of its
            own to glide along.
          </Paragraph>
        </>
      }
      demo={ClippedRevealCard}
    />
  );
}

function ClippedRevealCard() {
  const [open, setOpen] = useState(false);

  return (
    <node style={column}>
      <Button onClick={() => setOpen((v) => !v)}>
        {open ? "Hide details -" : "Show details +"}
      </Button>
      <node style={{ ...reveal, height: open ? "auto" : 1 }}>
        <Stage style={{ ...panel, opacity: open ? 1 : 0 }}>
          <Caption>My height is auto: the box</Caption>
          <Caption>snaps open, the clip eases</Caption>
          <Caption>with the box that is shown.</Caption>
        </Stage>
      </node>
      <Stage style={footer}>
        <Caption>I glide on my own layout entry.</Caption>
      </Stage>
    </node>
  );
}

const Reveal = { duration: 300, easing: "easeInOut" } as const;

// Holds the open height, so the card around the demo does not jump: the
// column's real layout snaps (only the two nodes with the channel ease).
const column: BevyStyle = {
  flexDirection: "column",
  gap: 10,
  width: 216,
  minHeight: 160,
};

const reveal: BevyStyle = {
  flexDirection: "column",
  justifyContent: "center",
  overflowY: "hidden",
  transition: { layout: Reveal },
};

// Fades with the box: collapsed, one pixel of the panel's middle is still
// inside the clip.
const panel: BevyStyle = {
  flexDirection: "column",
  flexShrink: 0,
  gap: 4,
  transition: { opacity: Reveal },
};

const footer: BevyStyle = {
  padding: { horizontal: 12, vertical: 6 },
  alignItems: "center",
  transition: { layout: Reveal },
};
