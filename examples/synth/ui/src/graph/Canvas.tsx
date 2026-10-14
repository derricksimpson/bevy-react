import { type Dispatch, useEffect, useMemo, useRef, useState } from "react";
import type { PointerEventData } from "bevy-react";
import { bevy, type Drag } from "../bevy";
import { AddMenu } from "../chrome/AddMenu";
import { useDebug, useLatest } from "../hooks";
import { SIGNAL, alpha } from "../theme";
import { ZoomContext } from "../widgets";
import { HEADER, type Kind, ROW, ROWS_PAD, kind } from "./catalog";
import {
  type Action,
  type Graph,
  type SocketRef,
  canPlug,
  live,
  nodeBox,
  signalOf,
  socketName,
} from "./model";
import { type CardApi, NodeCard, type Plug } from "./NodeCard";

/** The node every card is laid out in (`ROOT` in `view.rs`): Bevy owns its
 *  transform, gliding the view toward `view.set`. */
export const GRAPH_ROOT = "graph-root";

export type View = { x: number; y: number; zoom: number; glide: number };

/** Zoom levels are powers of this, so a zoom lays text out at one of a
 *  bounded set of sizes. */
const ZOOM_STEP = 1.12;
export const zoomLevel = (zoom: number) =>
  Math.max(-11, Math.min(7, Math.round(Math.log(zoom) / Math.log(ZOOM_STEP))));
export const zoomAt = (level: number) => ZOOM_STEP ** level;

type Vec = { x: number; y: number };
type Wire = {
  from: SocketRef;
  to: SocketRef | null;
  at: [number, number] | null;
};
type Menu = { x: number; y: number; wire: SocketRef | null };
type Gesture =
  | { kind: "pan"; from: Vec; view: View; moved: boolean; button: number }
  | { kind: "box"; from: Vec; base: string[] }
  | {
      kind: "move";
      from: Vec;
      start: Record<string, Vec>;
      moved: boolean;
      press: string;
      shift: boolean;
      was: boolean;
    };

const same = (a: SocketRef | null, b: SocketRef | null) =>
  a === b ||
  (!!a && !!b && a.node === b.node && a.side === b.side && a.port === b.port);

export function Canvas({
  graph,
  dispatch,
  view,
  setView,
  gliding,
  playing,
}: {
  graph: Graph;
  dispatch: Dispatch<Action>;
  view: View;
  setView: (v: View) => void;
  gliding: boolean;
  playing: boolean;
}) {
  const [wire, setWire] = useState<Wire | null>(null);
  const [lifted, setLifted] = useState<string[]>([]);
  const [box, setBox] = useState<{ a: Vec; b: Vec } | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const gesture = useRef<Gesture | null>(null);
  const wheel = useRef(0);
  const state = useLatest({ graph, view, wire });

  const toWorld = (p: Vec, v = state.current.view) => ({
    x: (p.x - v.x) / v.zoom,
    y: (p.y - v.y) / v.zoom,
  });
  const client = (e: PointerEventData) => ({ x: e.clientX, y: e.clientY });

  const pan = {
    start(e: PointerEventData) {
      gesture.current = {
        kind: "pan",
        from: client(e),
        view: state.current.view,
        moved: false,
        button: e.button ?? 0,
      };
    },
    move(e: PointerEventData) {
      const g = gesture.current;
      if (g?.kind !== "pan") return;
      const dx = e.clientX - g.from.x;
      const dy = e.clientY - g.from.y;
      if (!g.moved && Math.hypot(dx, dy) < 3) return;
      g.moved = true;
      setView({ ...g.view, x: g.view.x + dx, y: g.view.y + dy, glide: 0 });
    },
  };

  // Stable for the memoized cards; reads the latest state through `state`.
  const api = useMemo<CardApi>(
    () => ({
      press(id, e) {
        const { graph } = state.current;
        if (graph.nodes[id]?.leaving) return;
        setMenu(null);
        if (e.button === 1) return pan.start(e);
        if (e.button !== 0) return;
        const was = graph.selected.includes(id);
        const ids = was
          ? graph.selected
          : e.shiftKey
            ? [...graph.selected, id]
            : [id];
        if (!was) dispatch({ type: "select", ids });
        dispatch({ type: "raise", ids: [...ids.filter((i) => i !== id), id] });
        gesture.current = {
          kind: "move",
          from: client(e),
          start: Object.fromEntries(
            ids.map((i) => [i, { x: graph.nodes[i].x, y: graph.nodes[i].y }]),
          ),
          moved: false,
          press: id,
          shift: e.shiftKey,
          was,
        };
      },
      drag(e) {
        const g = gesture.current;
        if (g?.kind === "pan") return pan.move(e);
        if (g?.kind !== "move") return;
        const dx = e.clientX - g.from.x;
        const dy = e.clientY - g.from.y;
        if (!g.moved && Math.hypot(dx, dy) < 3) return;
        if (!g.moved) {
          g.moved = true;
          setLifted(Object.keys(g.start));
        }
        const zoom = state.current.view.zoom;
        const to: Record<string, Vec> = {};
        for (const [id, p] of Object.entries(g.start)) {
          to[id] = {
            x: Math.round(p.x + dx / zoom),
            y: Math.round(p.y + dy / zoom),
          };
        }
        dispatch({ type: "move", to });
      },
      release() {
        const g = gesture.current;
        gesture.current = null;
        if (g?.kind !== "move") return;
        setLifted([]);
        if (g.moved) return;
        // A click: just this node, or Shift toggles it.
        const { selected } = state.current.graph;
        if (!g.shift) dispatch({ type: "select", ids: [g.press] });
        else if (g.was)
          dispatch({
            type: "select",
            ids: selected.filter((i) => i !== g.press),
          });
      },
      socketDown(s, e) {
        if (e.button !== 0) return;
        setMenu(null);
        const { graph } = state.current;
        let from = s;
        // A wired input lets go of its wire: the hand takes it from there.
        const held =
          s.side === "i" &&
          graph.links.find((l) => l.to.node === s.node && l.to.port === s.port);
        if (held) {
          dispatch({ type: "unplug", id: held.id });
          from = { node: held.from.node, side: "o", port: held.from.port };
        }
        setWire({ from, to: null, at: null });
      },
      socketHover(s, on) {
        // Functional updates: an enter and a leave may land in one tick.
        const { graph } = state.current;
        setWire((w) => {
          if (!w || w.at) return w;
          if (!on) return same(w.to, s) ? { ...w, to: null } : w;
          return canPlug(graph, w.from, s) && !same(w.to, s)
            ? { ...w, to: s }
            : w;
        });
      },
      socketUp(e) {
        const { wire, graph } = state.current;
        if (!wire) return;
        if (wire.to) {
          dispatch({ type: "connect", a: wire.from, b: wire.to });
          setWire(null);
          return;
        }
        // Dropped on bare canvas: offer the modules that would fit.
        const p = toWorld(client(e));
        const onNode = live(graph).some((n) => {
          const b = nodeBox(n);
          return (
            p.x >= b.x && p.x <= b.x + b.w && p.y >= b.y && p.y <= b.y + b.h
          );
        });
        if (onNode) return setWire(null);
        setWire({ ...wire, at: [e.clientX, e.clientY] });
        setMenu({ x: e.clientX, y: e.clientY, wire: wire.from });
      },
      param(id, key, value) {
        dispatch({ type: "param", id, key, value });
      },
      purge(id) {
        dispatch({ type: "purge", id });
      },
    }),
    // `state`, `dispatch` and the setters are stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );
  const closeMenu = () => {
    setMenu(null);
    setWire(null);
  };
  const pick = (k: Kind) => {
    if (!menu) return;
    const p = toWorld(menu);
    const id = `n${graph.seq}`;
    const def = kind(k);
    const w = menu.wire;
    if (!w) {
      dispatch({ type: "add", kind: k, x: p.x, y: p.y, select: true });
    } else {
      // The socket that takes the wire lands under the drop point.
      const side = w.side === "o" ? "i" : "o";
      const ports = side === "i" ? def.inputs : def.outputs;
      const port = Math.max(
        0,
        ports.findIndex((q) => q.signal === signalOf(graph, w)),
      );
      dispatch({
        type: "add",
        kind: k,
        x: side === "i" ? p.x + 14 : p.x - def.width - 14,
        y: p.y - HEADER - ROWS_PAD - ROW * (port + 0.5),
        select: true,
      });
      dispatch({ type: "connect", a: w, b: { node: id, side, port } });
    }
    setMenu(null);
    setWire(null);
  };

  // Shift+A adds at the pointer (Bevy knows where it is: nothing here
  // tracks it); Escape lets go of whatever is open.
  useEffect(
    () =>
      bevy.on("keyDown", (e) => {
        if (e.repeat) return;
        if (e.key === "Escape") closeMenu();
        if (e.code === "KeyA" && e.shiftKey && !e.ctrlKey) {
          void bevy.pointer.get().then((at) => {
            const [x, y] = at ?? [400, 300];
            setMenu({ x, y, wire: null });
          });
        }
      }),
    [],
  );
  useDebug("menu", ([x, y]) =>
    setMenu({ x: Number(x), y: Number(y), wire: null }),
  );

  // Bevy hears the patch whenever what it draws changes (not on moves).
  const sent = useRef("");
  useEffect(() => {
    const selected = new Set(graph.selected);
    const drag: Drag | null = wire && {
      from: socketName(wire.from),
      output: wire.from.side === "o",
      to: wire.to && socketName(wire.to),
      at: wire.at,
      signal: signalOf(graph, wire.from),
      color: SIGNAL[signalOf(graph, wire.from)],
    };
    const msg = {
      wires: graph.links.map((l) => {
        const out: SocketRef = {
          node: l.from.node,
          side: "o",
          port: l.from.port,
        };
        const signal = signalOf(graph, out);
        return {
          id: l.id,
          from: socketName(out),
          to: socketName({ node: l.to.node, side: "i", port: l.to.port }),
          signal,
          color: SIGNAL[signal],
          hot: selected.has(l.from.node) || selected.has(l.to.node),
        };
      }),
      drag,
      playing,
    };
    const json = JSON.stringify(msg);
    if (json !== sent.current) {
      sent.current = json;
      bevy.patch.set(msg);
    }
  });

  const plugs = (id: string): Record<string, Plug> | null => {
    if (!wire) return null;
    const d = kind(graph.nodes[id].kind);
    const out: Record<string, Plug> = {};
    for (const [side, ports] of [
      ["i", d.inputs],
      ["o", d.outputs],
    ] as const) {
      ports.forEach((_, port) => {
        const s: SocketRef = { node: id, side, port };
        out[`${side}${port}`] =
          same(s, wire.to) || same(s, wire.from)
            ? "target"
            : canPlug(graph, wire.from, s)
              ? "open"
              : "shut";
      });
    }
    return out;
  };
  const wired = (id: string) =>
    graph.links
      .flatMap((l) => [
        l.from.node === id && `o${l.from.port}`,
        l.to.node === id && `i${l.to.port}`,
      ])
      .filter(Boolean)
      .join(" ");

  const selectBox = (a: Vec, b: Vec, base: string[]) => {
    const [p, q] = [toWorld(a), toWorld(b)];
    const x0 = Math.min(p.x, q.x);
    const x1 = Math.max(p.x, q.x);
    const y0 = Math.min(p.y, q.y);
    const y1 = Math.max(p.y, q.y);
    const hits = live(graph)
      .filter((n) => {
        const r = nodeBox(n);
        return r.x < x1 && r.x + r.w > x0 && r.y < y1 && r.y + r.h > y0;
      })
      .map((n) => n.id);
    dispatch({ type: "select", ids: [...new Set([...base, ...hits])] });
  };

  return (
    <node
      onPointerDown={(e) => {
        setMenu(null);
        if (wire?.at) setWire(null);
        if (e.button === 2)
          return setMenu({ x: e.clientX, y: e.clientY, wire: null });
        if (e.button === 0 && e.shiftKey) {
          gesture.current = {
            kind: "box",
            from: client(e),
            base: graph.selected,
          };
          return setBox({ a: client(e), b: client(e) });
        }
        pan.start(e);
      }}
      onPointerMove={(e) => {
        const g = gesture.current;
        if (g?.kind === "box") {
          setBox({ a: g.from, b: client(e) });
          selectBox(g.from, client(e), g.base);
        } else pan.move(e);
      }}
      onPointerUp={() => {
        const g = gesture.current;
        gesture.current = null;
        setBox(null);
        // A click on bare canvas lets go of the selection.
        if (g?.kind === "pan" && !g.moved && g.button === 0)
          dispatch({ type: "select", ids: [] });
      }}
      onWheel={(e) => {
        wheel.current += e.deltaMode === "line" ? e.deltaY : e.deltaY / 50;
        const steps = Math.trunc(wheel.current);
        if (!steps) return;
        wheel.current -= steps;
        const zoom = zoomAt(zoomLevel(view.zoom) + steps);
        const p = toWorld(client(e));
        setView({
          x: e.clientX - p.x * zoom,
          y: e.clientY - p.y * zoom,
          zoom,
          glide: 170,
        });
      }}
      style={{
        positionType: "absolute",
        left: 0,
        top: 0,
        width: "100%",
        height: "100%",
        overflowX: "clip",
        overflowY: "clip",
      }}
    >
      <ZoomContext.Provider value={view.zoom}>
        <node
          name={GRAPH_ROOT}
          style={{
            positionType: "absolute",
            left: 0,
            top: 0,
            width: 0,
            height: 0,
          }}
        >
          {Object.values(graph.nodes).map((n) => (
            <NodeCard
              key={n.id}
              node={n}
              selected={graph.selected.includes(n.id)}
              lifted={lifted.includes(n.id)}
              gliding={gliding}
              wired={wired(n.id)}
              plug={plugs(n.id)}
              api={api}
            />
          ))}
        </node>
      </ZoomContext.Provider>
      {box && (
        <node
          style={{
            positionType: "absolute",
            left: Math.min(box.a.x, box.b.x),
            top: Math.min(box.a.y, box.b.y),
            width: Math.abs(box.b.x - box.a.x),
            height: Math.abs(box.b.y - box.a.y),
            borderRadius: 4,
            border: 1,
            borderColor: alpha("#8fb4ff", 0.7),
            backgroundColor: alpha("#8fb4ff", 0.08),
          }}
        />
      )}
      {menu && (
        <AddMenu
          x={menu.x}
          y={menu.y}
          accepts={menu.wire ? accepts(graph, menu.wire) : undefined}
          onPick={pick}
          onClose={closeMenu}
        />
      )}
    </node>
  );
}

/** Kinds with a socket a wire from `s` could plug into. */
function accepts(graph: Graph, s: SocketRef) {
  const signal = signalOf(graph, s);
  return (k: Kind) =>
    (s.side === "o" ? kind(k).inputs : kind(k).outputs).some(
      (p) => p.signal === signal,
    );
}
