// The Animations tab: the shared-value table — every running driver (and,
// behind the "idle" chip, every idle value an app node reads) — with each
// driver stage's live phase and the nodes the value drives.
//
// Two clocks. The readouts arrive as `devtools.animations` events, sampled by
// Rust every 100 ms while this panel is mounted (the mount effect announces
// `devtools.animationsOpen`, its cleanup withdraws it — the LayersPanel
// contract). The meters don't wait for them: each is bound to its shared
// value itself, `{ animated }` like any app style, so the animation engine
// slides it every frame with no bridge traffic and no re-render.

import { useEffect, useState } from "react";
import {
  onAnimations,
  sendAnimationsOpen,
  sendHighlight,
  type DevtoolsAnimationRow,
  type DevtoolsAnimations,
} from "./api";
import { Chip, ScrollArea } from "./DevtoolsHost";
import { ReasonPill } from "./LayersPanel";
import { boundFields, mirror } from "./mirror";
import { theme } from "./theme";
import { rowLabel, useMirrorVersion } from "./TreeView";

/** The meter's width — px, because a bound length is always px. */
const METER = 72;

export function AnimationsPanel() {
  // Node labels and bound fields come from the mirror; re-render when it moves.
  useMirrorVersion();
  const [table, setTable] = useState<DevtoolsAnimations | null>(null);
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

  const values = table?.values ?? [];
  const running = values.filter((row) => row.driver.length > 0);
  const shown = idle ? values : running;
  const hidden = (table?.total ?? 0) - values.length;
  return (
    <>
      <node
        style={{
          flexDirection: "row",
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
        <Chip label="idle" active={idle} onClick={() => setIdle(!idle)} />
        <text style={{ color: theme.textDim, fontSize: 10 }}>
          {`${running.length} running, ${values.length - running.length} idle`}
        </text>
      </node>
      <ScrollArea>
        {!table ? (
          <Note text="waiting for animation data" />
        ) : shown.length === 0 ? (
          <Note
            text={idle ? "no running or bound values" : "nothing running"}
          />
        ) : (
          shown.map((row) => <AnimationRow key={row.id} row={row} />)
        )}
        {hidden > 0 && <Note text={`+${hidden} more`} />}
      </ScrollArea>
    </>
  );
}

function AnimationRow({ row }: { row: DevtoolsAnimationRow }) {
  const running = row.driver.length > 0;
  const mono = { fontSize: 10, fontFamily: theme.mono } as const;
  return (
    <node
      style={{
        flexDirection: "column",
        padding: { horizontal: 10, vertical: 4 },
        border: { bottom: 1 },
        borderColor: theme.border,
      }}
    >
      <node style={{ flexDirection: "row", alignItems: "center", gap: 6 }}>
        <node
          style={{
            width: 8,
            height: 8,
            flexShrink: 0,
            borderRadius: 4,
            backgroundColor: running ? theme.ok : theme.border,
          }}
        />
        <text
          style={{
            color: running ? theme.text : theme.textDim,
            fontSize: 11,
            fontFamily: theme.mono,
          }}
        >
          {`sv ${row.id}`}
        </text>
        {!running && <ReasonPill label="idle" />}
        <node style={{ flexGrow: 1 }} />
        {row.range && <Meter id={row.id} range={row.range} />}
        {/* A fixed, right-aligned column: the meters line up whatever the
            reading's width. */}
        <node style={{ width: 52, flexShrink: 0, justifyContent: "flexEnd" }}>
          <text style={{ color: theme.text, ...mono }}>
            {String(row.value)}
          </text>
        </node>
      </node>
      {/* Outermost stage first; each nested stage indents under its parent. */}
      {row.driver.map((line, i) => (
        <text
          key={i}
          style={{
            color: theme.textDim,
            ...mono,
            margin: { left: 14 + i * 10, top: 2 },
          }}
        >
          {line}
        </text>
      ))}
      {row.nodes.map((id) => (
        <BoundNode key={id} id={id} shared={row.id} />
      ))}
    </node>
  );
}

/** Where the reading sits in `range`, bound to the shared value: the engine
 *  slides the fill every frame — a translate, so no relayout — while the
 *  number beside it is a 100 ms sample. */
function Meter({ id, range }: { id: number; range: [number, number] }) {
  return (
    <node
      style={{
        width: METER,
        height: 4,
        flexShrink: 0,
        backgroundColor: theme.bgActive,
        overflowX: "hidden",
      }}
    >
      <node
        style={{
          width: METER,
          height: 4,
          backgroundColor: theme.accent,
          transform: {
            translateX: {
              animated: {
                type: "interpolate",
                id,
                input: range,
                output: [-METER, 0],
              },
            },
          },
        }}
      />
    </node>
  );
}

/** One app node the value drives, with the fields it binds. Hovering
 *  outlines the node on screen (the tree-row highlight channel). */
function BoundNode({ id, shared }: { id: number; shared: number }) {
  const node = mirror.get(id);
  const fields = node ? boundFields(node, shared).join(", ") : "";
  return (
    <node
      style={{
        flexDirection: "row",
        gap: 6,
        margin: { left: 14, top: 2 },
        padding: { horizontal: 4 },
        borderRadius: 3,
      }}
      hoverStyle={{ backgroundColor: theme.bgActive }}
      onPointerEnter={() => sendHighlight(id)}
      onPointerLeave={() => sendHighlight(null)}
    >
      <text
        style={{ color: theme.accent, fontSize: 10, fontFamily: theme.mono }}
      >
        {`${node ? rowLabel(node) : ""} #${id}`.trim()}
      </text>
      {fields !== "" && (
        <text
          style={{ color: theme.textDim, fontSize: 10, fontFamily: theme.mono }}
        >
          {fields}
        </text>
      )}
    </node>
  );
}

function Note({ text }: { text: string }) {
  return (
    <text style={{ color: theme.textDim, fontSize: 11, margin: 8 }}>
      {text}
    </text>
  );
}
