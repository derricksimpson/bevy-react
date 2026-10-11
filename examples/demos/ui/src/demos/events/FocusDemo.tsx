import { useState } from "react";
import { InlineCode, List, ListItem, Paragraph } from "@/components/typography";
import { BevyStyle } from "bevy-react/jsx";
import { DemoRow, Example, Readout } from "@/components";
import { Code } from "@/components/docs";
import { Colors, FontSizes } from "@/theme";
import { useDemoPage, type ExplanationData } from "@/explanationStore";

// Keyboard focus: `tabIndex` makes any element focusable, `onFocus`/`onBlur`
// report it, `focusStyle` shows it. The gallery adds Bevy's
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
      >
        <text style={chipLabelStyle}>Fire</text>
      </button>
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
  justifyContent: "center",
  padding: { horizontal: 12 },
  borderRadius: 10,
  border: 1,
  borderColor: Colors.lineStrong,
  backgroundColor: Colors.well,
  color: Colors.text,
  fontSize: FontSizes.sm,
  cursor: "text",
};

// Applied on the Bevy side while the element holds the focus.
const focusStyle: BevyStyle = {
  borderColor: Colors.cyan,
  boxShadow: { blurRadius: 3, color: Colors.cyanGlow },
};
