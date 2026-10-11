import { useState } from "react";
import { InlineCode, List, ListItem, Paragraph } from "@/components/typography";
import { BevyStyle } from "bevy-react/jsx";
import { Checkbox, DemoRow, Example, Readout } from "@/components";
import { Code } from "@/components/docs";
import { Colors, FontSizes } from "@/theme";
import { useDemoPage, type ExplanationData } from "@/explanationStore";

// Keyboard focus: `tabIndex` makes any element focusable, `onFocus`/`onBlur`
// report it, `focusStyle` shows it and `focusVisibleStyle` rings it under
// keyboard navigation. The gallery adds Bevy's
// `TabNavigationPlugin` (examples/demos/main.rs), so Tab walks the page.

const USAGE = `<button
  tabIndex={0}
  onFocus={() => setFocused("One")}
  onBlur={() => setFocused(null)}
  focusStyle={{
    borderColor: "#5cd9ff",
  }}
>
  <text>One</text>
</button>`;

const RING = `<button
  tabIndex={0}
  focusStyle={{ borderColor: "#5cd9ff" }}
  focusVisibleStyle={{
    outline: { width: 2, offset: 2 },
  }}
/>`;

const TAB_PLUGIN = `use bevy::input_focus::tab_navigation
  ::TabNavigationPlugin;

app.add_plugins(TabNavigationPlugin);`;

const PAGE: ExplanationData = {
  title: "Focus",
  info: (
    <>
      <Paragraph>
        One element at a time holds the keyboard focus. An{" "}
        <InlineCode>{"<editableText>"}</InlineCode> takes it on its own; any
        other element does with a <InlineCode>tabIndex</InlineCode>.{" "}
        <InlineCode>onFocus</InlineCode> and <InlineCode>onBlur</InlineCode>{" "}
        report the change, and <InlineCode>focusStyle</InlineCode> shows it with
        no React state.
      </Paragraph>
      <Code lang="tsx">{USAGE}</Code>
      <List>
        <ListItem>
          A press focuses the topmost focusable element under the pointer (a
          label focuses the button around it); a press anywhere else blurs.
        </ListItem>
        <ListItem>
          tabIndex 0 joins the Tab order, a negative value is focusable by click
          only.
        </ListItem>
        <ListItem>
          Tab and Shift+Tab move the focus once the app adds Bevy's
          TabNavigationPlugin — one line of Rust.
        </ListItem>
        <ListItem>Enter and Space click a focused {"<button>"}.</ListItem>
        <ListItem>
          focusVisibleStyle adds a ring only when the keyboard moved the focus
          (and always on a text field).
        </ListItem>
      </List>
      <Code lang="rust">{TAB_PLUGIN}</Code>
    </>
  ),
};

export function FocusDemo() {
  useDemoPage(PAGE);
  return (
    <DemoRow>
      <TabOrderExample />
      <ActivationExample />
      <DisabledExample />
    </DemoRow>
  );
}

function TabOrderExample() {
  return (
    <Example
      title="Tab order"
      info={
        <>
          <Paragraph>
            Two buttons with <InlineCode>tabIndex={"{0}"}</InlineCode>, one with{" "}
            <InlineCode>tabIndex={"{-1}"}</InlineCode> and a text field. Click
            one, or press Tab: the readout follows{" "}
            <InlineCode>onFocus</InlineCode> and <InlineCode>onBlur</InlineCode>
            , and the focused element lights up through its{" "}
            <InlineCode>focusStyle</InlineCode>. Tab skips the click-only
            button; a click on the card's background blurs.
          </Paragraph>
          <Code lang="tsx">{USAGE}</Code>
          <Paragraph>
            The outer ring is a <InlineCode>focusVisibleStyle</InlineCode>: it
            overlays <InlineCode>focusStyle</InlineCode> only while the focus
            indicator shows — after Tab, not after a click — and always on a
            focused text field, like CSS <InlineCode>:focus-visible</InlineCode>
            .
          </Paragraph>
          <Code lang="tsx">{RING}</Code>
        </>
      }
      demo={TabOrderCard}
    />
  );
}

function TabOrderCard() {
  const [focused, setFocused] = useState<string | null>(null);
  const [text, setText] = useState("");
  // The old element's `onBlur` fires before the new one's `onFocus`.
  const track = (label: string) => ({
    onFocus: () => setFocused(label),
    onBlur: () => setFocused(null),
  });
  return (
    <>
      <Readout
        label="Focused"
        value={focused ?? "nothing"}
        size={24}
        color={focused ? Colors.cyan : Colors.textDim}
      />
      <node style={rowStyle}>
        {["One", "Two"].map((label) => (
          <button
            key={label}
            tabIndex={0}
            {...track(label)}
            style={chipStyle}
            hoverStyle={chipHoverStyle}
            focusStyle={focusStyle}
            focusVisibleStyle={ringStyle}
          >
            <text style={chipLabelStyle}>{label}</text>
          </button>
        ))}
        <button
          tabIndex={-1}
          {...track("Click only")}
          style={chipStyle}
          hoverStyle={chipHoverStyle}
          focusStyle={focusStyle}
          focusVisibleStyle={ringStyle}
        >
          <text style={chipLabelStyle}>Click only</text>
        </button>
        <editableText
          value={text}
          onChange={setText}
          {...track("Field")}
          ariaLabel="Field"
          style={fieldStyle}
          focusStyle={focusStyle}
          focusVisibleStyle={ringStyle}
        />
      </node>
    </>
  );
}

const ACTIVATION = `<button
  tabIndex={0}
  onClick={() => setCount((c) => c + 1)}
>
  <text>Fire</text>
</button>`;

function ActivationExample() {
  return (
    <Example
      title="Keyboard activation"
      info={
        <>
          <Paragraph>
            A focused <InlineCode>{"<button>"}</InlineCode> clicks on Enter and
            Space: its <InlineCode>onClick</InlineCode> fires once per key
            press, as for a pointer click. Click the button (or Tab to it), then
            press either key. Only a <InlineCode>{"<button>"}</InlineCode> is
            activated this way, and only with a{" "}
            <InlineCode>tabIndex</InlineCode>: a button that can't be focused
            never takes the keys.
          </Paragraph>
          <Code lang="tsx">{ACTIVATION}</Code>
        </>
      }
      demo={ActivationCard}
    />
  );
}

function ActivationCard() {
  const [count, setCount] = useState(0);
  return (
    <>
      <Readout label="Clicks" value={count} />
      <button
        tabIndex={0}
        onClick={() => setCount((c) => c + 1)}
        style={chipStyle}
        hoverStyle={chipHoverStyle}
        focusStyle={focusStyle}
        focusVisibleStyle={ringStyle}
      >
        <text style={chipLabelStyle}>Fire</text>
      </button>
    </>
  );
}

const DISABLED = `<button tabIndex={0} disabled={off}>
  <text>Middle</text>
</button>
<editableText disabled={off} />`;

function DisabledExample() {
  return (
    <Example
      title="Disabled"
      info={
        <>
          <Paragraph>
            <InlineCode>disabled</InlineCode> on a{" "}
            <InlineCode>{"<button>"}</InlineCode> or an{" "}
            <InlineCode>{"<editableText>"}</InlineCode> takes it out of focus:
            while the box is checked, a click on the middle button or the field
            focuses nothing, Tab goes from First straight to Last, and the
            button doesn't click. Uncheck it and both join the row again. An
            element that becomes disabled while focused gives the focus up.
          </Paragraph>
          <Code lang="tsx">{DISABLED}</Code>
        </>
      }
      demo={DisabledCard}
    />
  );
}

function DisabledCard() {
  const [off, setOff] = useState(true);
  const [clicks, setClicks] = useState(0);
  const [text, setText] = useState("Field");
  const chip = off ? disabledChipStyle : chipStyle;
  return (
    <>
      <Readout label="Middle clicks" value={clicks} />
      <node style={rowStyle}>
        <button
          tabIndex={0}
          style={chipStyle}
          hoverStyle={chipHoverStyle}
          focusStyle={focusStyle}
          focusVisibleStyle={ringStyle}
        >
          <text style={chipLabelStyle}>First</text>
        </button>
        <button
          tabIndex={0}
          disabled={off}
          onClick={() => setClicks((c) => c + 1)}
          style={chip}
          hoverStyle={off ? undefined : chipHoverStyle}
          focusStyle={focusStyle}
          focusVisibleStyle={ringStyle}
        >
          <text style={off ? disabledLabelStyle : chipLabelStyle}>Middle</text>
        </button>
        <editableText
          value={text}
          onChange={setText}
          disabled={off}
          ariaLabel="Field"
          style={off ? disabledFieldStyle : fieldStyle}
          focusStyle={focusStyle}
          focusVisibleStyle={ringStyle}
        />
        <button
          tabIndex={0}
          style={chipStyle}
          hoverStyle={chipHoverStyle}
          focusStyle={focusStyle}
          focusVisibleStyle={ringStyle}
        >
          <text style={chipLabelStyle}>Last</text>
        </button>
      </node>
      <Checkbox label="disabled" enabled={off} onChange={setOff} />
    </>
  );
}

const rowStyle: BevyStyle = {
  flexDirection: "row",
  flexWrap: "wrap",
  justifyContent: "center",
  alignItems: "center",
  gap: 10,
  maxWidth: 420,
};

// A quiet chip: the focus light is the only accent.
const chipStyle: BevyStyle = {
  height: 36,
  padding: { horizontal: 14 },
  justifyContent: "center",
  alignItems: "center",
  borderRadius: 10,
  border: 1,
  borderColor: Colors.lineStrong,
  backgroundColor: Colors.well,
  cursor: "pointer",
};

const chipHoverStyle: BevyStyle = {
  backgroundColor: Colors.control,
};

const chipLabelStyle: BevyStyle = {
  fontSize: FontSizes.sm,
  fontWeight: "medium",
  color: Colors.text,
};

const fieldStyle: BevyStyle = {
  width: 140,
  height: 36,
  padding: { horizontal: 12, vertical: 8 },
  borderRadius: 10,
  border: 1,
  borderColor: Colors.lineStrong,
  backgroundColor: Colors.well,
  color: Colors.text,
  fontSize: FontSizes.sm,
  cursor: "text",
};

// `disabled` has no look of its own: the app dims the control.
const disabledChipStyle: BevyStyle = {
  ...chipStyle,
  borderColor: Colors.line,
  backgroundColor: Colors.raised,
  cursor: "notAllowed",
};

const disabledLabelStyle: BevyStyle = {
  ...chipLabelStyle,
  color: Colors.textFaint,
};

const disabledFieldStyle: BevyStyle = {
  ...fieldStyle,
  borderColor: Colors.line,
  backgroundColor: Colors.raised,
  color: Colors.textFaint,
  cursor: "notAllowed",
};

// Overlaid on `focusStyle` while the focus indicator shows: after Tab, and
// always on a focused text field.
const ringStyle: BevyStyle = {
  outline: { width: 2, offset: 2, color: Colors.cyanBright },
};

// Applied on the Bevy side while the element holds the focus.
const focusStyle: BevyStyle = {
  borderColor: Colors.cyan,
  boxShadow: { blurRadius: 3, color: Colors.cyanGlow },
};
