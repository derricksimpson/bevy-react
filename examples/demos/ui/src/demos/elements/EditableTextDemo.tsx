import { useState } from "react";
import { InlineCode, ListItem, Paragraph, List } from "@/components/typography";
import { BevyStyle } from "bevy-react/jsx";
import { DemoRow, Example } from "@/components";
import { Code } from "@/components/docs";
import { Colors, Fonts, FontSizes } from "@/theme";
import { useDemoPage, type ExplanationData } from "@/explanationStore";

const PAGE: ExplanationData = {
  title: "<editableText>",
  info: (
    <>
      <Paragraph>
        <InlineCode>{"<editableText>"}</InlineCode> is a focusable text input
        with the controlled-component contract you know from React:{" "}
        <InlineCode>value</InlineCode> + <InlineCode>onChange</InlineCode>.
        Editing, caret, selection, IME composition, and clipboard (Ctrl+C/V/X)
        are all handled engine-side.
      </Paragraph>
      <Code lang="tsx">{`<editableText
  value={name}
  onChange={setName}
  autofocus
  maxLength={40}
  style={inputStyle}
  focusStyle={{ borderColor: "#5cd9ff" }}
/>`}</Code>
      <List>
        <ListItem>
          focusStyle overlays while the field has focus — applied on the Bevy
          side, no onFocus/onBlur round-trip or React focus state needed.
        </ListItem>
        <ListItem>
          onFocus / onBlur / onSelect report focus and selection; the old
          field's onBlur fires before the new field's onFocus.
        </ListItem>
        <ListItem>
          maxLength caps input; autofocus grabs focus on mount.
        </ListItem>
        <ListItem>
          readOnly keeps the text selectable but unchangeable; disabled takes
          the field out of focus altogether.
        </ListItem>
      </List>
    </>
  ),
};

type Selection = {
  start: number;
  end: number;
  direction: string;
  composing: boolean;
};

export function EditableTextDemo() {
  useDemoPage(PAGE);
  return (
    <DemoRow>
      <NameFormExample />
      <ModesExample />
    </DemoRow>
  );
}

function NameFormExample() {
  return (
    <Example
      title="Name form"
      info={
        <>
          <Paragraph>
            Two controlled fields feeding one greeting, with a status box
            mirroring what the element reports: which field is focused, the
            caret or selection range, and whether an IME composition is in
            flight. Moving between the fields, the old one's{" "}
            <InlineCode>onBlur</InlineCode> arrives before the new one's{" "}
            <InlineCode>onFocus</InlineCode>, so a plain set and clear pair
            tracks the focused field.
          </Paragraph>
          <Code lang="tsx">{`<editableText
  value={first}
  onChange={setFirst}
  onFocus={() => setFocused("First name")}
  onBlur={() => setFocused(null)}
  onSelect={(s) => setSel(s)}
  autofocus
  style={inputStyle}
  focusStyle={{ borderColor: "#5cd9ff" }}
/>`}</Code>
        </>
      }
      demo={NameFormCard}
    />
  );
}

function ModesExample() {
  return (
    <Example
      title="Modes"
      info={
        <>
          <Paragraph>
            One text in three fields. The first edits it. The{" "}
            <InlineCode>readOnly</InlineCode> one still takes the focus, the
            caret and a selection you can copy, but no edit. The{" "}
            <InlineCode>disabled</InlineCode> one is inert: a click doesn't
            focus it, Tab skips it, and it shows no caret. Both follow the
            controlled <InlineCode>value</InlineCode>. There is no disabled
            look: dim the field through its <InlineCode>style</InlineCode>.
          </Paragraph>
          <Code lang="tsx">{`<editableText
  value={text}
  onChange={setText}
/>
<editableText value={text} readOnly />
<editableText
  value={text}
  disabled
  style={{ ...inputStyle, ...dimStyle }}
/>`}</Code>
        </>
      }
      demo={ModesCard}
    />
  );
}

function ModesCard() {
  const [text, setText] = useState("Sable City");
  return (
    <node style={formStyle}>
      <node style={fieldStyle}>
        <text style={labelStyle}>Editable</text>
        <editableText
          value={text}
          onChange={setText}
          ariaLabel="Editable"
          maxLength={40}
          style={inputStyle}
          focusStyle={focusStyle}
        />
      </node>
      <node style={fieldStyle}>
        <text style={labelStyle}>readOnly</text>
        <editableText
          value={text}
          readOnly
          ariaLabel="Read-only"
          style={inputStyle}
          focusStyle={focusStyle}
        />
      </node>
      <node style={fieldStyle}>
        <text style={labelStyle}>disabled</text>
        <editableText
          value={text}
          disabled
          ariaLabel="Disabled"
          style={{ ...inputStyle, ...dimStyle }}
          focusStyle={focusStyle}
        />
      </node>
    </node>
  );
}

function NameFormCard() {
  const [first, setFirst] = useState("");
  const [last, setLast] = useState("");
  const [focused, setFocused] = useState<string | null>(null);
  const [sel, setSel] = useState<Selection | null>(null);

  const name = [first, last].filter(Boolean).join(" ");

  // The old field's `onBlur` fires before the new field's `onFocus`.
  const blur = () => setFocused(null);
  const select = (s: {
    selectionStart: number;
    selectionEnd: number;
    selectionDirection: string;
    composing: boolean;
  }) =>
    setSel({
      start: s.selectionStart,
      end: s.selectionEnd,
      direction: s.selectionDirection,
      composing: s.composing,
    });

  return (
    <node style={formStyle}>
      <node style={fieldStyle}>
        <text style={labelStyle}>What's your first name?</text>
        <editableText
          value={first}
          onChange={setFirst}
          onFocus={() => setFocused("First name")}
          onBlur={blur}
          onSelect={select}
          autofocus
          ariaLabel="First name"
          maxLength={40}
          style={inputStyle}
          focusStyle={focusStyle}
        />
      </node>

      <node style={fieldStyle}>
        <text style={labelStyle}>What's your last name?</text>
        <editableText
          value={last}
          onChange={setLast}
          onFocus={() => setFocused("Last name")}
          onBlur={blur}
          onSelect={select}
          ariaLabel="Last name"
          maxLength={40}
          style={inputStyle}
          focusStyle={focusStyle}
        />
      </node>

      <text style={greetingStyle}>{name ? `Hello ${name}` : " "}</text>

      <node style={statusBoxStyle}>
        <text style={statusLineStyle}>Focused: {focused ?? "none"}</text>
        <text style={statusLineStyle}>
          {sel
            ? sel.start === sel.end
              ? `Caret at ${sel.start}`
              : `Selection ${sel.start}–${sel.end} (${sel.direction})`
            : "Selection: none"}
          {sel?.composing ? " · composing" : ""}
        </text>
      </node>
    </node>
  );
}

// The form's column: labels sit flush over their fields.
const formStyle: BevyStyle = {
  width: 280,
  flexDirection: "column",
  gap: 16,
};

const fieldStyle: BevyStyle = {
  flexDirection: "column",
  gap: 6,
};

const labelStyle: BevyStyle = {
  fontSize: FontSizes.sm,
  fontWeight: "medium",
  color: Colors.textBody,
};

// The house input: a recessed well with a hairline rim. The caret follows
// `color`.
const inputStyle: BevyStyle = {
  width: 280,
  height: 40,
  justifyContent: "center",
  padding: { horizontal: 12, vertical: 8 },
  backgroundColor: Colors.well,
  borderRadius: 10,
  border: 1,
  borderColor: Colors.line,
  color: Colors.text,
  fontSize: FontSizes.base,
};

// Overlaid on `inputStyle` while the field is focused — applied entirely on the
// Bevy side, so no `onFocus`/`onBlur` round-trip or React state is needed. The
// focused field lights up: a cyan rim and its glow.
const focusStyle: BevyStyle = {
  borderColor: Colors.cyan,
  boxShadow: { blurRadius: 3, color: Colors.cyanGlow },
};

// A disabled field has no look of its own: the app dims it.
const dimStyle: BevyStyle = {
  color: Colors.textFaint,
  backgroundColor: Colors.raised,
};

const greetingStyle: BevyStyle = {
  fontFamily: Fonts.display,
  fontSize: FontSizes.xxl,
  fontWeight: "semibold",
  color: Colors.text,
};

// What the element reports, as a readout.
const statusBoxStyle: BevyStyle = {
  flexDirection: "column",
  gap: 4,
  padding: { horizontal: 12, vertical: 8 },
  backgroundColor: Colors.raised,
  borderRadius: 10,
  border: 1,
  borderColor: Colors.line,
};

const statusLineStyle: BevyStyle = {
  fontFamily: Fonts.mono,
  fontSize: FontSizes.xs,
  color: Colors.textBody,
};
