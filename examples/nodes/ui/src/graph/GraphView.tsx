import { type MutableRefObject, useEffect, useRef, useState } from "react";
import type { PointerEventData } from "bevy-react";
import { bevy, type NodeDef } from "../bevy";
import { isTyping, useDebug } from "../hooks";
import { AddMenu, ADD_MENU_HEIGHT } from "../menus/AddMenu";
import { Menu, MENU_WIDTH, menuHeight } from "../menus/Menu";
import { anyPopup, openPopup } from "../menus/popup";
import { C, SOCKET } from "../theme";
import { type Commands, makeCommands } from "./commands";
import { useEditor } from "./editor";
import { MAX_ZOOM, MIN_ZOOM, type View, useInteraction } from "./interact";
import { type Box, type SocketRef, bounds, socketPos } from "./layout";
import { type NodeId, inputType, nodeList, outputType } from "./model";
import { NodeCard } from "./NodeCard";
import { Wire } from "./Wires";

/** Zoom steps on a 1/64 grid: every committed zoom renders text at one of a
 *  bounded set of sizes (each new size is new glyphs in the atlas). */
const snapZoom = (z: number) =>
  Math.round(Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z)) * 64) / 64;

/** After the wheel rests this long, the graph re-lays out at the new zoom. */
const COMMIT_MS = 160;

/** The node graph: grid, wires and node cards, panned and zoomed. While the
 *  wheel turns the content is scaled with one transform; once it rests,
 *  every length is re-laid out at the new zoom (`z`) so text stays crisp. */
export function GraphView({
  rect,
  pending,
  commands,
  onToggleSidebar,
}: {
  /** Where the view sits in the window (logical px). */
  rect: Box;
  pending: ReadonlySet<NodeId>;
  commands: MutableRefObject<Commands | null>;
  onToggleSidebar: () => void;
}) {
  const { state, dispatch } = useEditor();
  const [view, setViewState] = useState<View>({ x: 40, y: 40, zoom: 0.75 });
  const [z, setZ] = useState(0.75);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const setView = (v: View) => {
    const zoom = snapZoom(v.zoom);
    setViewState({ ...v, zoom });
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setZ(zoom), COMMIT_MS);
  };

  /** The Add menu at a viewport point; `connect` links the pick to a
   *  dropped wire's socket, else the new node follows the cursor. */
  const openAdd = (at: { x: number; y: number }, connect?: SocketRef) => {
    const accepts = connect
      ? (d: NodeDef) =>
          (connect.side === "output" ? d.inputs : d.outputs).length > 0
      : undefined;
    openPopup({
      x: rect.x + at.x - 20,
      y: rect.y + at.y - 10,
      w: MENU_WIDTH,
      h: ADD_MENU_HEIGHT,
      render: (close) => (
        <AddMenu
          close={close}
          accepts={accepts}
          onPick={(kind) =>
            connect
              ? ix.addConnected(kind, connect, ix.toGraph(at))
              : ix.addGrabbed(kind, ix.toGraph(ix.cursor.current))
          }
        />
      ),
    });
  };

  const ix = useInteraction(state, dispatch, view, setView, rect, {
    openAdd,
    openNodeMenu(id: NodeId, e: PointerEventData) {
      const node = state.doc.nodes[id];
      // Built when the menu renders: the press that opens it may have just
      // selected the node, and the commands act on the selection of the
      // render they come from.
      const items = () => {
        const c = commands.current!;
        return [
          { label: "Duplicate", hint: "Shift D", onSelect: c.duplicate },
          { label: "Copy", hint: "Ctrl C", onSelect: c.copy },
          { label: "Delete", hint: "X", onSelect: c.delete },
          {
            label: node?.mute ? "Unmute" : "Mute",
            hint: "M",
            onSelect: c.mute,
            separator: true,
          },
          {
            label: node?.collapsed ? "Expand" : "Collapse",
            hint: "H",
            onSelect: c.collapse,
          },
          {
            label: "Toggle Preview",
            onSelect: c.preview,
            disabled: !c.canPreview(),
          },
        ];
      };
      openPopup({
        x: e.clientX,
        y: e.clientY,
        w: MENU_WIDTH,
        h: menuHeight(items()) + 20,
        render: (close) => <Menu title="Node" items={items()} close={close} />,
      });
    },
  });

  const frame = (ids?: NodeId[]) => {
    const doc = state.doc;
    const b = bounds(doc, ids ?? nodeList(doc).map((n) => n.id));
    if (!b) return;
    const zoom = snapZoom(
      Math.min(1.25, (rect.w - 80) / b.w, (rect.h - 60) / b.h),
    );
    setViewState({
      x: rect.w / 2 - (b.x + b.w / 2) * zoom,
      y: rect.h / 2 - (b.y + b.h / 2) * zoom,
      zoom,
    });
    setZ(zoom);
  };
  const cmd = makeCommands(
    ix,
    () => state,
    dispatch,
    frame,
    () => ({ x: rect.w / 2, y: rect.h / 2 }),
  );
  commands.current = cmd;

  // A loaded preset fits the view once its nodes exist.
  const framed = useRef<unknown>(null);
  useEffect(() => {
    if (framed.current !== state.preset && nodeList(state.doc).length) {
      framed.current = state.preset;
      frame();
    }
  });

  // The shortcuts. Every key arrives here; a field being typed into or an
  // open menu keeps them.
  const latest = useRef({ cmd, openAdd, onToggleSidebar });
  latest.current = { cmd, openAdd, onToggleSidebar };
  useEffect(
    () =>
      bevy.on("keyDown", (e) => {
        if (isTyping() || anyPopup()) return;
        const c = latest.current.cmd;
        if (ix.grabbing()) {
          if (e.code === "Escape") ix.endGrab(false);
          if (e.code === "Enter" || e.code === "NumpadEnter") ix.endGrab(true);
          return;
        }
        const ctrl = e.ctrlKey || e.metaKey;
        if (e.repeat && !(ctrl && (e.code === "KeyZ" || e.code === "KeyY")))
          return;
        if (ctrl) {
          if (e.code === "KeyZ") (e.shiftKey ? c.redo : c.undo)();
          else if (e.code === "KeyY") c.redo();
          else if (e.code === "KeyC") c.copy();
          else if (e.code === "KeyV") c.paste();
          return;
        }
        switch (e.code) {
          case "KeyA":
            if (e.shiftKey) latest.current.openAdd(ix.cursor.current);
            else if (e.altKey) c.deselect();
            else c.selectAll();
            break;
          case "KeyD":
            if (e.shiftKey) c.duplicate();
            break;
          case "KeyG":
            c.grab();
            break;
          case "KeyX":
          case "Delete":
            c.delete();
            break;
          case "KeyM":
            c.mute();
            break;
          case "KeyH":
            c.collapse();
            break;
          case "Home":
            c.frameAll();
            break;
          case "KeyF":
          case "NumpadDecimal":
          case "Period":
            c.frameSelected();
            break;
          case "KeyN":
            latest.current.onToggleSidebar();
            break;
        }
      }),
    // `latest`/`ix` are stable; the handler reads through them.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );
  useDebug("view", ([x, y, zoom]) => {
    const v = { x: Number(x), y: Number(y), zoom: Number(zoom) };
    setViewState(v);
    setZ(v.zoom);
  });
  useDebug("frame", () => frame());
  useDebug("menu", ([at]) => {
    if (at === "add") openAdd({ x: rect.w * 0.45, y: rect.h * 0.35 });
  });

  const doc = state.doc;
  const selected = new Set(state.selection);
  const linked = new Map<NodeId, string[]>();
  for (const l of doc.links)
    linked.set(l.to, [...(linked.get(l.to) ?? []), l.input]);
  const nodes = nodeList(doc);
  const scale = view.zoom / z;

  return (
    <node
      style={{
        width: rect.w,
        height: rect.h,
        overflowX: "clip",
        overflowY: "clip",
        backgroundColor: C.graph,
      }}
      onPointerDown={ix.background.down}
      onPointerMove={ix.api.move}
      onPointerUp={ix.api.release}
      onPointerHover={ix.background.hover}
      onWheel={ix.background.wheel}
    >
      <grid
        offset={[view.x, view.y]}
        spacing={20 * view.zoom}
        color={C.grid}
        style={{
          positionType: "absolute",
          left: 0,
          top: 0,
          width: rect.w,
          height: rect.h,
        }}
      />
      <node
        style={{
          positionType: "absolute",
          left: view.x,
          top: view.y,
          width: 0,
          height: 0,
          transform: { scale },
        }}
      >
        {doc.links.map((l) => {
          const from = doc.nodes[l.from];
          const to = doc.nodes[l.to];
          if (!from || !to) return null;
          const a = socketPos(from, "output", l.output);
          const b = socketPos(to, "input", l.input);
          const lit = selected.has(l.from) || selected.has(l.to);
          const dim = from.mute ? "80" : "";
          return (
            <Wire
              key={`${l.to}:${l.input}`}
              ax={a.x}
              ay={a.y}
              bx={b.x}
              by={b.y}
              z={z}
              color={SOCKET[outputType(doc, l.from, l.output)] + dim}
              colorTo={SOCKET[inputType(doc, l.to, l.input)] + dim}
              thickness={lit ? 3.2 : 2.4}
            />
          );
        })}
        {ix.visuals.liveWire && (
          <Wire
            ax={ix.visuals.liveWire.a.x}
            ay={ix.visuals.liveWire.a.y}
            bx={ix.visuals.liveWire.b.x}
            by={ix.visuals.liveWire.b.y}
            z={z}
            color={wireColor(doc, ix.visuals.liveWire.fixed)}
            thickness={2.8}
          />
        )}
        {nodes.map((n) => (
          <NodeCard
            key={n.id}
            node={n}
            z={z}
            selected={selected.has(n.id)}
            active={state.active === n.id}
            pending={pending.has(n.id)}
            linked={(linked.get(n.id) ?? []).join(",")}
            types={n.kind === "reroute" ? outputType(doc, n.id, "out") : "any"}
            api={ix.api}
          />
        ))}
        {ix.visuals.stroke &&
          ix.visuals.stroke.points.slice(1).map((p, i) => {
            const q = ix.visuals.stroke!.points[i];
            return (
              <Wire
                key={i}
                ax={q.x}
                ay={q.y}
                bx={p.x}
                by={p.y}
                z={z}
                straight
                dash={8}
                thickness={1.6}
                color={
                  ix.visuals.stroke!.tool === "knife" ? C.knife : "#ffffff"
                }
              />
            );
          })}
      </node>
      {ix.visuals.box && (
        <node
          style={{
            positionType: "absolute",
            left: ix.visuals.box.x,
            top: ix.visuals.box.y,
            width: ix.visuals.box.w,
            height: ix.visuals.box.h,
            border: 1,
            borderColor: C.boxBorder,
            backgroundColor: C.box,
          }}
        />
      )}
      {ix.visuals.grabbing && (
        <node
          style={{
            positionType: "absolute",
            left: 0,
            top: 0,
            width: rect.w,
            height: rect.h,
          }}
          onPointerDown={ix.background.grabPress}
          onPointerHover={ix.background.hover}
        />
      )}
      <text
        style={{
          positionType: "absolute",
          left: 10,
          bottom: 6,
          fontSize: 11,
          color: C.textMuted,
        }}
      >
        {`${Math.round(view.zoom * 100)}%  ·  ${nodes.length} nodes`}
      </text>
    </node>
  );
}

function wireColor(
  doc: Parameters<typeof outputType>[0],
  s: SocketRef,
): string {
  return SOCKET[
    s.side === "output"
      ? outputType(doc, s.node, s.id)
      : inputType(doc, s.node, s.id)
  ];
}
