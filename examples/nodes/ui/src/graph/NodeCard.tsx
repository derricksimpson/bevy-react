import { memo, useRef, useState } from "react";
import type { PointerEventData } from "bevy-react";
import type { SocketType, Value } from "../bevy";
import { def, valueOf } from "../catalog";
import { ParamField } from "../fields/ParamField";
import { TextEdit } from "../fields/TextEdit";
import { Icon } from "../icons";
import { C, CATEGORY, G, SOCKET } from "../theme";
import { geometry } from "./layout";
import type { GNode, NodeId } from "./model";

/** What a node card asks of the graph view. Stable for the view's life, so
 *  memoized cards don't re-render for it. */
export interface GraphApi {
  pressNode(id: NodeId, e: PointerEventData): void;
  pressSocket(
    id: NodeId,
    side: "input" | "output",
    socket: string,
    e: PointerEventData,
  ): void;
  /** The move/release of whatever gesture a press started. */
  move(e: PointerEventData): void;
  release(e: PointerEventData): void;
  setValue(id: NodeId, key: string, value: Value, record: boolean): void;
  patch(id: NodeId, patch: Partial<GNode>): void;
}

function Socket({
  type,
  side,
  z,
  top,
  onPress,
  api,
}: {
  type: SocketType;
  side: "input" | "output";
  z: number;
  /** Its center's height in the card (graph units). */
  top: number;
  onPress: (e: PointerEventData) => void;
  api: GraphApi;
}) {
  const hit = 16;
  const dot = G.socket;
  return (
    <node
      style={{
        positionType: "absolute",
        top: (top - hit / 2) * z,
        [side === "input" ? "left" : "right"]: (-hit / 2) * z,
        width: hit * z,
        height: hit * z,
        alignItems: "center",
        justifyContent: "center",
      }}
      onPointerDown={onPress}
      onPointerMove={api.move}
      onPointerUp={api.release}
    >
      <node
        style={{
          width: dot * z,
          height: dot * z,
          borderRadius: (dot / 2) * z,
          backgroundColor: SOCKET[type],
          border: Math.max(1, z),
          borderColor: "#111111",
        }}
        hoverStyle={{ border: Math.max(2, 2 * z), borderColor: "#ffffff" }}
      />
    </node>
  );
}

function Row({ z, children }: { z: number; children: React.ReactNode }) {
  return (
    <node
      style={{
        height: G.row * z,
        flexDirection: "row",
        alignItems: "center",
        padding: { horizontal: 9 * z },
      }}
    >
      {children}
    </node>
  );
}

const label = (text: string, z: number, right = false) => (
  <text
    style={{
      fontSize: G.font * z,
      color: C.text,
      lineBreak: "noWrap",
      flexGrow: 1,
      textAlign: right ? "right" : "left",
    }}
  >
    {text}
  </text>
);

export const NodeCard = memo(function NodeCard({
  node,
  z,
  selected,
  active,
  pending,
  linked,
  types,
  api,
}: {
  node: GNode;
  z: number;
  selected: boolean;
  active: boolean;
  /** Bevy is still evaluating it. */
  pending: boolean;
  /** Its linked inputs, comma-joined (a string, so memo compares it). */
  linked: string;
  /** A reroute's type, following its source. */
  types: SocketType;
  api: GraphApi;
}) {
  const d = def(node.kind);
  const g = geometry(node, d);
  const [renaming, setRenaming] = useState(false);
  const lastPress = useRef(0);
  const isLinked = new Set(linked ? linked.split(",") : []);
  const w = g.w * z;
  const outline = selected ? (active ? C.active : C.selected) : C.nodeBorder;
  const press = (e: PointerEventData) => api.pressNode(node.id, e);

  if (node.kind === "reroute") {
    return (
      <node
        style={{
          positionType: "absolute",
          left: node.x * z,
          top: node.y * z,
          width: w,
          height: g.h * z,
          borderRadius: (G.reroute / 2) * z,
          backgroundColor: SOCKET[types],
          // An outline takes no space: the sockets stay on the true edges.
          outline: {
            width: selected ? 2 : 1,
            color: selected ? outline : "#111111",
          },
        }}
        onPointerDown={press}
        onPointerMove={api.move}
        onPointerUp={api.release}
      >
        <Socket
          type={types}
          side="input"
          z={z}
          top={G.reroute / 2}
          api={api}
          onPress={(e) => api.pressSocket(node.id, "input", "in", e)}
        />
        <Socket
          type={types}
          side="output"
          z={z}
          top={G.reroute / 2}
          api={api}
          onPress={(e) => api.pressSocket(node.id, "output", "out", e)}
        />
      </node>
    );
  }

  const set = (key: string) => (value: Value, record: boolean) =>
    api.setValue(node.id, key, value, record);
  const fieldWidth = g.w - 18;
  const header = (
    <node
      style={{
        height: G.header * z,
        flexDirection: "row",
        alignItems: "center",
        gap: 5 * z,
        padding: { horizontal: 7 * z },
        backgroundColor: node.mute ? C.muted : CATEGORY[d.category].header,
        borderRadius: node.collapsed
          ? (G.radius - 1) * z
          : {
              top: (G.radius - 1) * z,
              right: (G.radius - 1) * z,
              bottom: 0,
              left: 0,
            },
      }}
      onPointerDown={(e) => {
        // A double click renames.
        const now = Date.now();
        if (e.button === 0 && now - lastPress.current < 350) setRenaming(true);
        lastPress.current = now;
        press(e);
      }}
      onPointerMove={api.move}
      onPointerUp={api.release}
    >
      <node
        style={{
          width: 12 * z,
          height: 12 * z,
          alignItems: "center",
          justifyContent: "center",
        }}
        onClick={() => api.patch(node.id, { collapsed: !node.collapsed })}
      >
        <Icon
          name={node.collapsed ? "chevronRight" : "chevronDown"}
          size={10 * z}
        />
      </node>
      {renaming ? (
        <TextEdit
          initial={node.label ?? d.label}
          style={{
            fontSize: G.font * z,
            height: 18 * z,
            padding: { horizontal: 4 * z },
            borderRadius: 3 * z,
          }}
          onDone={(text) => {
            setRenaming(false);
            if (text !== null)
              api.patch(node.id, { label: text.trim() || undefined });
          }}
        />
      ) : (
        <text
          style={{
            fontSize: G.font * z,
            color: "#f2f2f2",
            lineBreak: "noWrap",
            flexGrow: 1,
            textShadow: { color: "rgba(0,0,0,0.6)", offsetY: 1 },
          }}
        >
          {node.label ?? d.label}
        </text>
      )}
      {pending && (
        <node
          style={{
            width: 6 * z,
            height: 6 * z,
            borderRadius: 3 * z,
            backgroundColor: "#ffd166",
          }}
        />
      )}
      {d.preview && !node.collapsed && (
        <node
          onClick={() => api.patch(node.id, { hidePreview: !node.hidePreview })}
        >
          <Icon
            name="eye"
            size={11 * z}
            color={node.hidePreview ? "#ffffff66" : "#ffffffcc"}
          />
        </node>
      )}
    </node>
  );

  const interpolation = d.params.find((p) => p.id === "interpolation");
  const constant =
    !!interpolation &&
    valueOf(node.values, interpolation.id, interpolation.field) === "constant";
  return (
    <node
      style={{
        positionType: "absolute",
        left: node.x * z,
        top: node.y * z,
        width: w,
        height: g.h * z,
        flexDirection: "column",
        borderRadius: G.radius * z,
        backgroundColor: node.mute ? "#2a2626" : C.node,
        // An outline takes no space: wires meet the sockets on the true edges.
        outline: { width: selected ? 2 : 1, color: outline },
        boxShadow: {
          color: "rgba(0,0,0,0.45)",
          blurRadius: 10 * z,
          yOffset: 3 * z,
        },
      }}
      onPointerDown={press}
      onPointerMove={api.move}
      onPointerUp={api.release}
    >
      {header}
      {node.collapsed ? (
        <>
          {d.inputs.map((s) => (
            <Socket
              key={`i-${s.id}`}
              type={s.type}
              side="input"
              z={z}
              top={G.header / 2}
              api={api}
              onPress={(e) => api.pressSocket(node.id, "input", s.id, e)}
            />
          ))}
          {d.outputs.map((s) => (
            <Socket
              key={`o-${s.id}`}
              type={s.type}
              side="output"
              z={z}
              top={G.header / 2}
              api={api}
              onPress={(e) => api.pressSocket(node.id, "output", s.id, e)}
            />
          ))}
        </>
      ) : (
        <node
          style={{
            flexDirection: "column",
            padding: { vertical: (G.pad / 2) * z },
          }}
        >
          {d.outputs.map((s) => (
            <Row key={s.id} z={z}>
              {label(s.label, z, true)}
              <Socket
                type={s.type}
                side="output"
                z={z}
                top={G.row / 2}
                api={api}
                onPress={(e) => api.pressSocket(node.id, "output", s.id, e)}
              />
            </Row>
          ))}
          {g.preview !== null && (
            <node
              style={{
                height: (G.thumb + G.pad) * z,
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <portal
                target={`node-${node.id}`}
                style={{
                  width: G.thumb * z,
                  height: G.thumb * z,
                  backgroundColor: "#141414",
                }}
              />
            </node>
          )}
          {d.params.map((p) =>
            p.field.kind === "ramp" ? (
              <node key={p.id} style={{ padding: { horizontal: 9 * z } }}>
                <ParamField
                  label={p.label}
                  field={p.field}
                  value={valueOf(node.values, p.id, p.field)}
                  z={z}
                  width={fieldWidth}
                  constant={constant}
                  onChange={set(p.id)}
                />
              </node>
            ) : (
              <Row key={p.id} z={z}>
                <ParamField
                  label={p.label}
                  field={p.field}
                  value={valueOf(node.values, p.id, p.field)}
                  z={z}
                  width={fieldWidth}
                  onChange={set(p.id)}
                />
              </Row>
            ),
          )}
          {d.inputs.map((s) => (
            <Row key={s.id} z={z}>
              <Socket
                type={s.type}
                side="input"
                z={z}
                top={G.row / 2}
                api={api}
                onPress={(e) => api.pressSocket(node.id, "input", s.id, e)}
              />
              {isLinked.has(s.id) || !s.field ? (
                label(s.label, z)
              ) : (
                <ParamField
                  label={s.label}
                  field={s.field}
                  value={valueOf(node.values, s.id, s.field)}
                  z={z}
                  width={fieldWidth}
                  onChange={set(s.id)}
                />
              )}
            </Row>
          ))}
        </node>
      )}
    </node>
  );
});
