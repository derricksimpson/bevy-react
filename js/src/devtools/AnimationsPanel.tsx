// The Animations tab: the shared-value table — every running driver (and,
// behind the "idle" chip, every idle value an app node reads) — and every
// node with transitions in flight, plus the playback controls: pause (also
// the devtools pause key, F11 by default) and speed drive the app clock, and
// a paused tab scrubs each run on its own.
//
// Two clocks. The readouts arrive as `devtools.animations` events, sampled by
// Rust every 100 ms of real time while this panel is mounted (the mount
// effect announces `devtools.animationsOpen`, its cleanup withdraws it — the
// LayersPanel contract; Rust hands the app clock back on that withdrawal).
// The meters don't wait for them: each is bound to its shared value itself,
// `{ animated }` like any app style, so the animation engine slides it every
// frame with no bridge traffic and no re-render.

import { useEffect, useState, type ReactNode } from "react";
import {
  onAnimations,
  sendAnimationsClock,
  sendAnimationsOpen,
  sendHighlight,
  type DevtoolsAnimations,
} from "./api";
import { TransitionRow, ValueRow } from "./AnimationRows";
import { Chip, ScrollArea } from "./DevtoolsHost";
import { theme } from "./theme";
import { useMirrorVersion } from "./TreeView";

const SPEEDS = [0.1, 0.25, 0.5, 1, 2];

export function AnimationsPanel() {
  // Node labels and bound fields come from the mirror; re-render when it moves.
  useMirrorVersion();
  const [table, setTable] = useState<DevtoolsAnimations | null>(null);
  const [values, setValues] = useState(true);
  const [transitions, setTransitions] = useState(true);
  // Idle bound values (finished, hover-driven, set from Rust) are opt-in:
  // the tab is about what is running.
  const [idle, setIdle] = useState(false);

  // Subscribe FIRST, then announce — the first payload must not race the
  // listener. The cleanup is the tab-hidden signal (and clears a highlight
  // left showing by a hovered node row).
  useEffect(() => {
    const off = onAnimations(setTable);
    sendAnimationsOpen(true);
    return () => {
      off();
      sendAnimationsOpen(false);
      sendHighlight(null);
    };
  }, []);

  // Rust's clock, never a local guess: the pause key and the app itself
  // move it too, and the stream reports any change the next frame.
  const clock = table?.clock ?? { paused: false, speed: 1 };
  const setClock = sendAnimationsClock;
  const rows = table?.values ?? [];
  const running = rows.filter((row) => row.driver.length > 0);
  const shown = idle ? rows : running;
  const nodes = table?.transitions ?? [];
  return (
    <>
      <node
        style={{
          flexDirection: "row",
          flexWrap: "wrap",
          alignItems: "center",
          gap: 6,
          padding: { horizontal: 6, vertical: 4 },
          border: { bottom: 1 },
          borderColor: theme.border,
          // Blocks scrolled-out rows stacked beneath (see Header note in
          // DevtoolsHost).
          focusPolicy: "block",
        }}
      >
        <Chip
          label={clock.paused ? "resume" : "pause"}
          active={clock.paused}
          onClick={() => setClock(!clock.paused, clock.speed)}
        />
        {SPEEDS.map((speed) => (
          <Chip
            key={speed}
            label={`${speed}x`}
            active={clock.speed === speed}
            onClick={() => setClock(clock.paused, speed)}
          />
        ))}
        <node style={{ width: 1, height: 12, backgroundColor: theme.border }} />
        <Chip
          label="values"
          active={values}
          onClick={() => setValues(!values)}
        />
        <Chip
          label="transitions"
          active={transitions}
          onClick={() => setTransitions(!transitions)}
        />
        {/* Values only: inert (and dim) while values are filtered out. */}
        <Chip
          label="idle"
          active={values && idle}
          onClick={() => values && setIdle(!idle)}
        />
      </node>
      <ScrollArea>
        {!table ? (
          <Note text="waiting for animation data" />
        ) : (
          <>
            {values && (
              <Section
                title={`Shared values: ${running.length} running, ${rows.length - running.length} idle`}
              >
                {shown.length === 0 ? (
                  <Note
                    text={
                      idle ? "no running or bound values" : "nothing running"
                    }
                  />
                ) : (
                  shown.map((row) => (
                    <ValueRow key={row.id} row={row} paused={clock.paused} />
                  ))
                )}
                <More hidden={table.total - rows.length} />
              </Section>
            )}
            {transitions && (
              <Section title={`Transitions: ${table.transitions_total} nodes`}>
                {nodes.length === 0 ? (
                  <Note text="nothing in flight" />
                ) : (
                  nodes.map((row) => (
                    <TransitionRow
                      key={row.node}
                      row={row}
                      paused={clock.paused}
                    />
                  ))
                )}
                <More hidden={table.transitions_total - nodes.length} />
              </Section>
            )}
          </>
        )}
      </ScrollArea>
    </>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <node style={{ flexDirection: "column" }}>
      <node
        style={{
          padding: { horizontal: 10, vertical: 3 },
          backgroundColor: theme.bgAlt,
          border: { bottom: 1 },
          borderColor: theme.border,
        }}
      >
        <text style={{ color: theme.textDim, fontSize: 10 }}>{title}</text>
      </node>
      {children}
    </node>
  );
}

function More({ hidden }: { hidden: number }) {
  return hidden > 0 ? <Note text={`+${hidden} more`} /> : null;
}

function Note({ text }: { text: string }) {
  return (
    <text style={{ color: theme.textDim, fontSize: 11, margin: 8 }}>
      {text}
    </text>
  );
}
