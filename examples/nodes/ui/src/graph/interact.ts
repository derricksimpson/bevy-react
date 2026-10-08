import { type Dispatch, useMemo, useRef, useState } from "react";
import type { PointerEventData, WheelEventData } from "bevy-react";
import { def } from "../catalog";
import { G } from "../theme";
import { endTyping } from "../hooks";
import type { Action, EditorState, SelectMode } from "./editor";
import {
  type Box,
  type P,
  type SocketRef,
  boxFrom,
  crossings,
  geometry,
  nearestSocket,
  nodesInBox,
  socketPos,
} from "./layout";
import {
  type Doc,
  type GNode,
  type NodeId,
  addNode,
  connect,
  disconnect,
  linkInto,
  linkKey,
  moveNodes,
  setValue,
  updateNodes,
} from "./model";
import type { GraphApi } from "./NodeCard";

export interface View {
  /** Pan: where graph (0, 0) sits in the viewport, logical px. */
  x: number;
  y: number;
  zoom: number;
}

export const MIN_ZOOM = 0.2;
export const MAX_ZOOM = 2.5;

/** Pixels a press may wander before it counts as a drag. */
const SLOP = 3;

type Gesture =
  | { kind: "none" }
  | {
      kind: "nodes";
      id: NodeId;
      start: Map<NodeId, P>;
      from: P;
      moved: boolean;
      recorded: boolean;
      shift: boolean;
    }
  | { kind: "pan"; last: P }
  | { kind: "wire"; fixed: SocketRef; snap: SocketRef | null }
  | { kind: "box"; from: P; mode: SelectMode }
  | { kind: "stroke"; tool: "knife" | "reroute" };

/** Nodes following the cursor with no button held (G, Shift+D, a node just
 *  added): a press confirms, right-click or Escape puts them back. */
interface Grab {
  start: Map<NodeId, P>;
  /** The cursor where it began, graph units. */
  from: P;
  /** The first move still has to record the undo step (G on placed nodes;
   *  a node just added or duplicated was recorded by its creation). */
  record: boolean;
}

/** A wire being dragged: from a fixed socket to the cursor or a snap. */
export interface LiveWire {
  a: P;
  b: P;
  fixed: SocketRef;
}

export interface Visuals {
  liveWire: LiveWire | null;
  /** The box being selected, viewport px. */
  box: Box | null;
  stroke: { tool: "knife" | "reroute"; points: P[] } | null;
  grabbing: boolean;
}

export interface Hooks {
  /** Open the Add menu at a viewport point; `connect` links the pick. */
  openAdd(at: P, connect?: SocketRef): void;
  /** Open a node's context menu. */
  openNodeMenu(id: NodeId, e: PointerEventData): void;
}

/** The graph view's input handling: pointer gestures, the wheel, grab mode
 *  and the editing commands, all reading the latest state through a ref so
 *  the handlers handed to memoized cards stay stable. */
export function useInteraction(
  state: EditorState,
  dispatch: Dispatch<Action>,
  view: View,
  setView: (v: View) => void,
  rect: Box,
  hooks: Hooks,
) {
  const [visuals, setVisuals] = useState<Visuals>({
    liveWire: null,
    box: null,
    stroke: null,
    grabbing: false,
  });
  const live = useRef({ state, view, setView, rect, hooks });
  live.current = { state, view, setView, rect, hooks };
  const gesture = useRef<Gesture>({ kind: "none" });
  const grab = useRef<Grab | null>(null);
  const strokePts = useRef<P[]>([]);
  /** The last cursor position, viewport px. */
  const cursor = useRef<P>({ x: rect.w / 2, y: rect.h / 2 });

  const tools = useMemo(() => {
    const L = () => live.current;
    const setView = (v: View) => L().setView(v);
    const local = (e: { clientX: number; clientY: number }): P => ({
      x: e.clientX - L().rect.x,
      y: e.clientY - L().rect.y,
    });
    const toGraph = (p: P): P => {
      const v = L().view;
      return { x: (p.x - v.x) / v.zoom, y: (p.y - v.y) / v.zoom };
    };
    const edit = (fn: (doc: Doc) => Doc, record = true, select?: NodeId[]) =>
      dispatch({ type: "edit", edit: fn, record, select });
    const select = (ids: NodeId[], mode: SelectMode, active?: NodeId | null) =>
      dispatch({ type: "select", ids, mode, active });
    const patch = (
      fn: (n: GNode) => Partial<GNode>,
      ids = L().state.selection,
    ) => edit((doc) => updateNodes(doc, ids, fn));

    /** Pick nodes up. `positions` are where they are (for nodes not yet in
     *  the rendered state, just added); without them, G on placed nodes. */
    const startGrab = (ids: NodeId[], positions?: Map<NodeId, P>) => {
      const doc = L().state.doc;
      const start =
        positions ??
        new Map(
          ids
            .filter((id) => doc.nodes[id])
            .map((id) => [id, { x: doc.nodes[id].x, y: doc.nodes[id].y }]),
        );
      if (!start.size) return;
      grab.current = {
        start,
        from: toGraph(cursor.current),
        record: !positions,
      };
      setVisuals((v) => ({ ...v, grabbing: true }));
    };
    const endGrab = (confirm: boolean) => {
      const g = grab.current;
      if (!g) return;
      if (!confirm) edit((doc) => moveNodes(doc, g.start), false);
      grab.current = null;
      setVisuals((v) => ({ ...v, grabbing: false }));
    };
    const grabMove = () => {
      const g = grab.current;
      if (!g) return;
      const p = toGraph(cursor.current);
      const d = { x: p.x - g.from.x, y: p.y - g.from.y };
      const positions = new Map(
        [...g.start].map(([id, s]) => [id, { x: s.x + d.x, y: s.y + d.y }]),
      );
      edit((doc) => moveNodes(doc, positions), g.record);
      g.record = false;
    };

    /** Add a node with its top left at a graph point; returns its id. */
    const add = (kind: string, at: P): NodeId => {
      const id = L().state.doc.nextId;
      edit((doc) => addNode(doc, kind, at.x, at.y)[0], true, [id]);
      return id;
    };

    /** Add a node centered on a graph point, following the cursor until a
     *  press places it (the Add menu's pick). */
    const addGrabbed = (kind: string, at: P) => {
      const g = geometry({ id: 0, kind, x: 0, y: 0, values: {} });
      const corner = { x: at.x - g.w / 2, y: at.y - G.header / 2 };
      const id = add(kind, corner);
      startGrab([id], new Map([[id, corner]]));
    };

    /** Add a node for a wire dropped at a graph point and link it: placed
     *  beside the drop with the linked socket at the cursor, Blender-like.
     *  Links the first socket of the other side, preferring the same type. */
    const addConnected = (kind: string, fixed: SocketRef, at: P) => {
      const d = def(kind);
      const doc = L().state.doc;
      const fixedNode = doc.nodes[fixed.node];
      if (!fixedNode) return;
      const fixedDef = def(fixedNode.kind);
      const fixedType = (
        fixed.side === "output" ? fixedDef.outputs : fixedDef.inputs
      ).find((s) => s.id === fixed.id)?.type;
      const pool = fixed.side === "output" ? d.inputs : d.outputs;
      const target = pool.find((s) => s.type === fixedType) ?? pool[0];
      if (!target) return;
      const probe: GNode = { id: 0, kind, x: 0, y: 0, values: {} };
      const g = geometry(probe, d);
      const socketY =
        (fixed.side === "output" ? g.inputs : g.outputs).get(target.id) ?? 0;
      const x = fixed.side === "output" ? at.x + 24 : at.x - g.w - 24;
      const id = add(kind, { x, y: at.y - socketY });
      const link =
        fixed.side === "output"
          ? { from: fixed.node, output: fixed.id, to: id, input: target.id }
          : { from: id, output: target.id, to: fixed.node, input: fixed.id };
      edit((d) => connect(d, link), false);
    };

    const pan = (e: PointerEventData) => {
      gesture.current = { kind: "pan", last: { x: e.clientX, y: e.clientY } };
    };

    const api: GraphApi = {
      pressNode(id, e) {
        endTyping();
        if (grab.current) return;
        if (e.button === 1) return pan(e);
        const { state } = L();
        if (e.button === 2) {
          if (!state.selection.includes(id)) select([id], "set", id);
          return L().hooks.openNodeMenu(id, e);
        }
        if (e.button !== 0) return;
        // Blender's clicks: Shift adds (or makes active, or drops the
        // active one); a plain press on a selected node keeps the selection
        // so it can be dragged together.
        let ids: NodeId[];
        if (e.shiftKey) {
          const removing = state.selection.includes(id) && state.active === id;
          select([id], removing ? "remove" : "add", removing ? undefined : id);
          ids = removing
            ? state.selection.filter((n) => n !== id)
            : [...new Set([...state.selection, id])];
        } else if (!state.selection.includes(id)) {
          select([id], "set", id);
          ids = [id];
        } else {
          select([], "add", id);
          ids = state.selection;
        }
        const doc = state.doc;
        gesture.current = {
          kind: "nodes",
          id,
          start: new Map(
            ids
              .filter((n) => doc.nodes[n])
              .map((n) => [n, { x: doc.nodes[n].x, y: doc.nodes[n].y }]),
          ),
          from: { x: e.clientX, y: e.clientY },
          moved: false,
          recorded: false,
          shift: e.shiftKey,
        };
      },
      pressSocket(id, side, socket, e) {
        endTyping();
        if (grab.current) return;
        if (e.button !== 0) return api.pressNode(id, e);
        const doc = L().state.doc;
        let fixed: SocketRef = { node: id, side, id: socket };
        const existing =
          side === "input" ? linkInto(doc, id, socket) : undefined;
        if (existing) {
          // Pick the link up from its input: re-route or drop it.
          edit((d) => disconnect(d, [linkKey(existing)]));
          fixed = { node: existing.from, side: "output", id: existing.output };
        }
        gesture.current = { kind: "wire", fixed, snap: null };
        api.move(e);
      },
      move(e) {
        const g = gesture.current;
        cursor.current = local(e);
        if (g.kind === "nodes") {
          const z = L().view.zoom;
          const dx = (e.clientX - g.from.x) / z;
          const dy = (e.clientY - g.from.y) / z;
          if (!g.moved && Math.hypot(dx * z, dy * z) < SLOP) return;
          g.moved = true;
          const positions = new Map(
            [...g.start].map(([id, s]) => [id, { x: s.x + dx, y: s.y + dy }]),
          );
          edit((doc) => moveNodes(doc, positions), !g.recorded);
          g.recorded = true;
        } else if (g.kind === "pan") {
          const v = L().view;
          setView({
            ...v,
            x: v.x + e.clientX - g.last.x,
            y: v.y + e.clientY - g.last.y,
          });
          g.last = { x: e.clientX, y: e.clientY };
        } else if (g.kind === "wire") {
          const doc = L().state.doc;
          const p = toGraph(cursor.current);
          const other = g.fixed.side === "output" ? "input" : "output";
          g.snap = nearestSocket(
            doc,
            p,
            other,
            24 / L().view.zoom,
            g.fixed.node,
          );
          const fixedNode = doc.nodes[g.fixed.node];
          if (!fixedNode) return;
          const fixedAt = socketPos(fixedNode, g.fixed.side, g.fixed.id);
          const end = g.snap
            ? socketPos(doc.nodes[g.snap.node], g.snap.side, g.snap.id)
            : p;
          const [a, b] =
            g.fixed.side === "output" ? [fixedAt, end] : [end, fixedAt];
          setVisuals((v) => ({ ...v, liveWire: { a, b, fixed: g.fixed } }));
        } else if (g.kind === "box") {
          setVisuals((v) => ({ ...v, box: boxFrom(g.from, cursor.current) }));
        } else if (g.kind === "stroke") {
          const p = toGraph(cursor.current);
          const pts = strokePts.current;
          const last = pts[pts.length - 1];
          if (
            !last ||
            Math.hypot(p.x - last.x, p.y - last.y) * L().view.zoom > 5
          ) {
            strokePts.current = [...pts, p];
            setVisuals((v) => ({
              ...v,
              stroke: { tool: g.tool, points: strokePts.current },
            }));
          }
        }
      },
      release(e) {
        const g = gesture.current;
        gesture.current = { kind: "none" };
        const { state } = L();
        if (g.kind === "nodes") {
          if (!g.moved && !g.shift && state.selection.length > 1)
            select([g.id], "set", g.id);
        } else if (g.kind === "wire") {
          setVisuals((v) => ({ ...v, liveWire: null }));
          if (g.snap) {
            const s = g.snap;
            const link =
              g.fixed.side === "output"
                ? {
                    from: g.fixed.node,
                    output: g.fixed.id,
                    to: s.node,
                    input: s.id,
                  }
                : {
                    from: s.node,
                    output: s.id,
                    to: g.fixed.node,
                    input: g.fixed.id,
                  };
            edit((doc) => connect(doc, link));
          } else {
            L().hooks.openAdd(local(e), g.fixed);
          }
        } else if (g.kind === "box") {
          setVisuals((v) => ({ ...v, box: null }));
          const b = boxFrom(g.from, local(e));
          if (b.w < SLOP && b.h < SLOP) {
            if (g.mode === "set") select([], "set", null);
            return;
          }
          const tl = toGraph({ x: b.x, y: b.y });
          const z = L().view.zoom;
          select(
            nodesInBox(state.doc, { x: tl.x, y: tl.y, w: b.w / z, h: b.h / z }),
            g.mode,
          );
        } else if (g.kind === "stroke") {
          const pts = strokePts.current;
          strokePts.current = [];
          setVisuals((v) => ({ ...v, stroke: null }));
          if (pts.length < 2) return;
          const hits = crossings(state.doc, pts);
          if (!hits.size) return;
          if (g.tool === "knife") {
            edit((doc) => disconnect(doc, [...hits.keys()]));
          } else {
            edit((doc) => {
              let d = doc;
              for (const [key, at] of hits) {
                const l = d.links.find((x) => linkKey(x) === key);
                if (!l) continue;
                const [next, id] = addNode(d, "reroute", at.x - 8, at.y - 8);
                d = disconnect(next, [key]);
                d = connect(d, {
                  from: l.from,
                  output: l.output,
                  to: id,
                  input: "in",
                });
                d = connect(d, {
                  from: id,
                  output: "out",
                  to: l.to,
                  input: l.input,
                });
              }
              return d;
            });
          }
        }
      },
      setValue(id, key, value, record) {
        edit((doc) => setValue(doc, id, key, value), record);
      },
      patch(id, p) {
        patch(() => p, [id]);
      },
    };

    const background = {
      down(e: PointerEventData) {
        endTyping();
        cursor.current = local(e);
        if (grab.current) return;
        if (e.button === 1) return pan(e);
        if (e.button === 2) {
          if (e.ctrlKey || e.shiftKey) {
            strokePts.current = [toGraph(cursor.current)];
            gesture.current = {
              kind: "stroke",
              tool: e.ctrlKey ? "knife" : "reroute",
            };
          } else {
            L().hooks.openAdd(cursor.current);
          }
          return;
        }
        if (e.button !== 0) return;
        gesture.current = {
          kind: "box",
          from: cursor.current,
          mode: e.shiftKey ? "add" : e.ctrlKey ? "remove" : "set",
        };
      },
      hover(e: PointerEventData) {
        cursor.current = local(e);
        grabMove();
      },
      wheel(e: WheelEventData) {
        const v = L().view;
        const p = local(e);
        if (e.deltaMode === "pixel" && !e.ctrlKey) {
          setView({ ...v, x: v.x + e.deltaX, y: v.y + e.deltaY });
          return;
        }
        if (e.ctrlKey && e.deltaMode === "line") {
          setView({ ...v, x: v.x + e.deltaY * 40 });
          return;
        }
        if (e.shiftKey && e.deltaMode === "line") {
          setView({ ...v, y: v.y + e.deltaY * 40 });
          return;
        }
        const steps = e.deltaMode === "line" ? e.deltaY : e.deltaY / 50;
        const zoom = Math.min(
          MAX_ZOOM,
          Math.max(MIN_ZOOM, v.zoom * 1.15 ** steps),
        );
        const k = zoom / v.zoom;
        setView({ x: p.x - (p.x - v.x) * k, y: p.y - (p.y - v.y) * k, zoom });
        cursor.current = p;
      },
      /** The grab overlay's press: left confirms, right cancels. */
      grabPress(e: PointerEventData) {
        endGrab(e.button !== 2);
      },
    };

    return {
      api,
      background,
      cursor,
      toGraph,
      startGrab,
      endGrab,
      grabbing: () => grab.current !== null,
      addGrabbed,
      addConnected,
      edit,
      select,
      patch,
    };
    // Everything reads the latest state through `live`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return { ...tools, visuals };
}

export type Interaction = ReturnType<typeof useInteraction>;
