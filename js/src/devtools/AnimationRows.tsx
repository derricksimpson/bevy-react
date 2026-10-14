// The Animations tab's rows: a shared value (its driver stages, meter and
// bound nodes) and a node's transitions in flight (one line per
// `transition` key). While the app clock is paused each running row gets a
// scrubber that moves it.

import { useEffect, useRef, useState } from "react";
import type { PointerEventData } from "../jsx";
import {
  sendHighlight,
  sendSeekRun,
  sendSeekValue,
  type DevtoolsAnimationRow,
  type DevtoolsTransitionRow,
} from "./api";
import { ReasonPill } from "./LayersPanel";
import { boundFields, mirror } from "./mirror";
import { theme } from "./theme";
import { rowLabel } from "./TreeView";

/** The meter's width — px, because a bound length is always px. */
const METER = 72;

const mono = { fontSize: 10, fontFamily: theme.mono } as const;

export function ValueRow({
  row,
  paused,
}: {
  row: DevtoolsAnimationRow;
  paused: boolean;
}) {
  const running = row.driver.length > 0;
  const scrub = paused && row.time !== null;
  return (
    <node style={rowStyle}>
      <node style={{ flexDirection: "row", alignItems: "center", gap: 6 }}>
        <Dot on={running} />
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
        {row.range && !scrub && <Meter id={row.id} range={row.range} />}
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
      {scrub && row.time && (
        <Scrubber time={row.time} onSeek={(t) => sendSeekValue(row.id, t)} />
      )}
      {row.nodes.map((id) => (
        <NodeLink key={id} id={id} fields={boundFieldsOf(id, row.id)} />
      ))}
    </node>
  );
}

export function TransitionRow({
  row,
  paused,
}: {
  row: DevtoolsTransitionRow;
  paused: boolean;
}) {
  return (
    <node style={rowStyle}>
      <node style={{ flexDirection: "row", alignItems: "center", gap: 6 }}>
        <Dot on />
        <NodeLink id={row.node} fields="" indent={0} />
      </node>
      {row.lines.map((line) => (
        <node key={line.label} style={{ flexDirection: "column" }}>
          <text
            style={{
              color: theme.textDim,
              ...mono,
              margin: { left: 14, top: 2 },
            }}
          >
            {paused
              ? `${line.label}: ${line.spec}`
              : `${line.label}: ${line.spec}, ${percent(line.time)}%`}
          </text>
          {paused && (
            <Scrubber
              time={line.time}
              onSeek={(t) => sendSeekRun(row.node, line.label, t)}
            />
          )}
        </node>
      ))}
    </node>
  );
}

const rowStyle = {
  flexDirection: "column",
  padding: { horizontal: 10, vertical: 4 },
  border: { bottom: 1 },
  borderColor: theme.border,
} as const;

function Dot({ on }: { on: boolean }) {
  return (
    <node
      style={{
        width: 8,
        height: 8,
        flexShrink: 0,
        borderRadius: 4,
        backgroundColor: on ? theme.ok : theme.border,
      }}
    />
  );
}

/** How far through its stretch a timeline is, floored (a run a hair from
 *  its end must not read 100%). */
function percent([at, start, end]: [number, number, number]): number {
  return end > start ? Math.floor(((at - start) / (end - start)) * 100) : 100;
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

/** A paused run's timeline: pressing or dragging the track moves the run to
 *  the pointed time. The event's `x` is normalized within the track and the
 *  runtime keeps a pressed node's moves coming window-wide, clamped at its
 *  edges. The thumb follows the pointer locally until the stream (100 ms
 *  samples) has caught up. */
function Scrubber({
  time,
  onSeek,
}: {
  time: [number, number, number];
  onSeek: (t: number) => void;
}) {
  const [at, start, end] = time;
  const span = end - start;
  const dragging = useRef(false);
  const [local, setLocal] = useState<number | null>(null);
  useEffect(() => {
    if (!dragging.current) setLocal(null);
  }, [at, start, end]);
  const seek = (e: PointerEventData) => {
    const t = start + Math.min(1, Math.max(0, e.x)) * span;
    setLocal(t);
    onSeek(t);
  };
  const shown = local ?? at;
  const fill = span > 0 ? ((shown - start) / span) * 100 : 100;
  return (
    <node
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 6,
        margin: { left: 14, top: 2 },
      }}
    >
      <node
        style={{
          flexGrow: 1,
          height: 12,
          flexDirection: "column",
          justifyContent: "center",
        }}
        onPointerDown={(e) => {
          if (e.button !== 0 || span <= 0) return;
          dragging.current = true;
          seek(e);
        }}
        onPointerMove={(e) => {
          if (dragging.current) seek(e);
        }}
        onPointerUp={() => {
          dragging.current = false;
        }}
      >
        <node style={{ height: 4, backgroundColor: theme.bgActive }}>
          <node
            style={{
              width: `${fill}%`,
              height: 4,
              backgroundColor: theme.accent,
            }}
          />
        </node>
      </node>
      <node style={{ width: 84, flexShrink: 0, justifyContent: "flexEnd" }}>
        <text style={{ color: theme.text, ...mono }}>
          {`${ms(shown - start)} / ${ms(span)} ms`}
        </text>
      </node>
    </node>
  );
}

const ms = (secs: number) => String(Math.round(secs * 1000));

function boundFieldsOf(node: number, shared: number): string {
  const n = mirror.get(node);
  return n ? boundFields(n, shared).join(", ") : "";
}

/** A node the row drives, with the fields it binds. Hovering outlines the
 *  node on screen (the tree-row highlight channel). */
function NodeLink({
  id,
  fields,
  indent = 14,
}: {
  id: number;
  fields: string;
  indent?: number;
}) {
  const node = mirror.get(id);
  return (
    <node
      style={{
        flexDirection: "row",
        gap: 6,
        margin: { left: indent, top: 2 },
        padding: { horizontal: 4 },
        borderRadius: 3,
      }}
      hoverStyle={{ backgroundColor: theme.bgActive }}
      onPointerEnter={() => sendHighlight(id)}
      onPointerLeave={() => sendHighlight(null)}
    >
      <text style={{ color: theme.accent, ...mono }}>
        {`${node ? rowLabel(node) : ""} #${id}`.trim()}
      </text>
      {fields !== "" && (
        <text style={{ color: theme.textDim, ...mono }}>{fields}</text>
      )}
    </node>
  );
}
