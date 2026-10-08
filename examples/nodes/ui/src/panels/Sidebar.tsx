import { useState, type ReactNode } from "react";
import type { Value } from "../bevy";
import { def, valueOf } from "../catalog";
import { BoolField } from "../fields/choice";
import { ParamField } from "../fields/ParamField";
import { TextEdit } from "../fields/TextEdit";
import { useEditor } from "../graph/editor";
import { linkInto, nodeList, setValue, updateNodes } from "../graph/model";
import { C, CATEGORY, UI } from "../theme";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <node
      style={{
        flexDirection: "column",
        gap: 4,
        padding: { horizontal: 10, vertical: 8 },
      }}
    >
      <text
        style={{
          fontSize: UI.small,
          color: C.textMuted,
          margin: { bottom: 2 },
        }}
      >
        {title}
      </text>
      {children}
    </node>
  );
}

function Line({ children }: { children: ReactNode }) {
  return (
    <node
      style={{ height: 22, flexDirection: "row", alignItems: "center", gap: 8 }}
    >
      {children}
    </node>
  );
}

/** The N-panel: the active node's name, toggles and every field, wider
 *  than the node itself; the graph's totals when nothing is active. */
export function Sidebar({
  width,
  height,
  timings,
  pending,
}: {
  width: number;
  height: number;
  timings: ReadonlyMap<number, number>;
  pending: ReadonlySet<number>;
}) {
  const { state, dispatch } = useEditor();
  const [renaming, setRenaming] = useState(false);
  const node =
    state.active !== null ? state.doc.nodes[state.active] : undefined;
  const fieldWidth = width - 20;
  const set = (key: string) => (value: Value, record: boolean) =>
    node &&
    dispatch({
      type: "edit",
      record,
      edit: (doc) => setValue(doc, node.id, key, value),
    });
  const toggle = (key: "mute" | "hidePreview" | "collapsed") => (on: boolean) =>
    node &&
    dispatch({
      type: "edit",
      record: true,
      edit: (doc) => updateNodes(doc, [node.id], () => ({ [key]: on })),
    });

  let body: ReactNode;
  if (!node) {
    const total = [...timings.values()].reduce((a, b) => a + b, 0);
    body = (
      <Section title="Graph">
        <text style={{ fontSize: UI.font, color: C.text }}>
          {`${nodeList(state.doc).length} nodes · ${state.doc.links.length} links`}
        </text>
        <text
          style={{ fontSize: UI.font, color: C.textDim }}
        >{`Full evaluation ${total.toFixed(1)} ms`}</text>
        <text
          style={{ fontSize: UI.small, color: C.textMuted, margin: { top: 8 } }}
        >
          Click a node to edit it here.
        </text>
      </Section>
    );
  } else {
    const d = def(node.kind);
    const ms = timings.get(node.id);
    body = (
      <>
        <Section title="Node">
          <Line>
            <text style={{ fontSize: UI.font, color: C.textDim, width: 44 }}>
              Name
            </text>
            {renaming ? (
              <TextEdit
                initial={node.label ?? d.label}
                style={{
                  fontSize: UI.font,
                  height: 20,
                  padding: { horizontal: 6 },
                  borderRadius: 4,
                }}
                onDone={(text) => {
                  setRenaming(false);
                  if (text !== null)
                    dispatch({
                      type: "edit",
                      record: true,
                      edit: (doc) =>
                        updateNodes(doc, [node.id], () => ({
                          label: text.trim() || undefined,
                        })),
                    });
                }}
              />
            ) : (
              <node
                style={{
                  flexGrow: 1,
                  height: 20,
                  borderRadius: 4,
                  backgroundColor: C.fieldDark,
                  padding: { horizontal: 6 },
                  alignItems: "center",
                }}
                hoverStyle={{ backgroundColor: "#242424" }}
                onClick={() => setRenaming(true)}
              >
                <text style={{ fontSize: UI.font, color: C.text }}>
                  {node.label ?? d.label}
                </text>
              </node>
            )}
          </Line>
          <Line>
            <node
              style={{
                width: 8,
                height: 8,
                borderRadius: 4,
                backgroundColor: CATEGORY[d.category].header,
              }}
            />
            <text style={{ fontSize: UI.small, color: C.textDim, flexGrow: 1 }}>
              {`${d.label} · ${CATEGORY[d.category].label}`}
            </text>
            <text
              style={{
                fontSize: UI.small,
                color: pending.has(node.id) ? "#ffd166" : C.textMuted,
              }}
            >
              {pending.has(node.id)
                ? "evaluating…"
                : ms !== undefined
                  ? `${ms.toFixed(1)} ms`
                  : ""}
            </text>
          </Line>
          <Line>
            <BoolField
              label="Mute"
              value={!!node.mute}
              z={1}
              onChange={toggle("mute")}
            />
            <BoolField
              label="Collapse"
              value={!!node.collapsed}
              z={1}
              onChange={toggle("collapsed")}
            />
            {d.preview && (
              <BoolField
                label="Preview"
                value={!node.hidePreview}
                z={1}
                onChange={(on) => toggle("hidePreview")(!on)}
              />
            )}
          </Line>
        </Section>
        {(d.params.length > 0 || d.inputs.some((s) => s.field)) && (
          <Section title="Properties">
            {d.params.map((p) => (
              <node
                key={p.id}
                style={{
                  minHeight: 22,
                  flexDirection: "column",
                  justifyContent: "center",
                }}
              >
                <ParamField
                  label={p.label}
                  field={p.field}
                  value={valueOf(node.values, p.id, p.field)}
                  z={1}
                  width={fieldWidth}
                  constant={node.values.interpolation === "constant"}
                  onChange={set(p.id)}
                />
              </node>
            ))}
            {d.inputs
              .filter((s) => s.field && !linkInto(state.doc, node.id, s.id))
              .map((s) => (
                <Line key={s.id}>
                  <ParamField
                    label={s.label}
                    field={s.field!}
                    value={valueOf(node.values, s.id, s.field!)}
                    z={1}
                    width={fieldWidth}
                    onChange={set(s.id)}
                  />
                </Line>
              ))}
          </Section>
        )}
      </>
    );
  }

  return (
    <node
      style={{
        width,
        height,
        flexDirection: "column",
        backgroundColor: C.pane,
        borderRadius: 4,
        overflowY: "scroll",
      }}
    >
      <node
        style={{
          height: UI.paneHeader,
          flexShrink: 0,
          flexDirection: "row",
          alignItems: "center",
          padding: { horizontal: 10 },
          backgroundColor: C.paneHeader,
          borderRadius: { top: 4, right: 4, bottom: 0, left: 0 },
        }}
      >
        <text style={{ fontSize: UI.font, color: C.text }}>
          {node ? "Node" : "Sidebar"}
        </text>
        <node style={{ flexGrow: 1 }} />
        <text style={{ fontSize: UI.small, color: C.textMuted }}>N</text>
      </node>
      {body}
    </node>
  );
}
