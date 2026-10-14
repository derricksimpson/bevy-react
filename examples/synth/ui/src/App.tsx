import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { useSharedValue, withRepeat, withTiming } from "bevy-react";
import { bevy } from "./bevy";
import { menuOpen } from "./chrome/AddMenu";
import { Minimap } from "./chrome/Minimap";
import { PALETTE_W, Palette } from "./chrome/Palette";
import { Hints, TopBar } from "./chrome/TopBar";
import { Canvas, type View } from "./graph/Canvas";
import { type Kind, KINDS, cardHeight } from "./graph/catalog";
import {
  EMPTY,
  type GraphNode,
  arrange,
  bounds,
  live,
  reduce,
} from "./graph/model";
import { PRESETS, WIRE_DELAY, instantiate } from "./graph/presets";
import { useDebug, useEvent, useLatest, useWindowSize } from "./hooks";
import { LiveContext } from "./widgets";

/** Where the graph shows: right of the palette, under the top bar. */
const INSET = { left: PALETTE_W + 28, top: 84, right: 24, bottom: 60 };

export function App() {
  const [graph, dispatch] = useReducer(reduce, EMPTY);
  const [view, setView] = useState<View>({ x: 600, y: 400, zoom: 1, glide: 0 });
  const [playing, setPlaying] = useState(true);
  const [bpm, setBpm] = useState(112);
  const [preset, setPreset] = useState(0);
  const [gliding, setGliding] = useState(false);
  const size = useWindowSize();
  const latest = useLatest({ graph, view, size });

  // The transport: eighth notes, two bars round. Bevy drives it from where
  // the sound is (`synth/mod.rs`), so every light shows what plays.
  const steps = useSharedValue(0);
  const spin = useSharedValue(0);
  // Stable between renders: a pan re-renders App every frame, and a fresh
  // value would re-render every card and scope that reads it.
  const ctx = useMemo(
    () => ({ playing, bpm, steps, spin }),
    [playing, bpm, steps, spin],
  );
  useEffect(() => {
    spin.value = withRepeat(withTiming(360, { duration: 4000 }));
  }, [spin]);

  useEffect(() => bevy.view.set(view), [view]);

  // Bevy plays the patch: it hears what it is made of whenever that
  // changes — a knob, a cable, the transport — never on a move.
  const sound = useRef("");
  useEffect(() => {
    const msg = {
      modules: live(graph).map(({ id, kind, params }) => ({
        id,
        kind,
        params,
      })),
      cables: graph.links.map(({ from, to }) => ({ from, to })),
      playing,
      bpm,
      lights: steps.id,
    };
    const json = JSON.stringify(msg);
    if (json !== sound.current) {
      sound.current = json;
      bevy.synth.set(msg);
    }
  });

  /** A view that shows `nodes` whole, at most at zoom 1. */
  const framing = (nodes: GraphNode[], glide: number): View | null => {
    const b = bounds(nodes);
    if (!b) return null;
    const { size } = latest.current;
    const w = size.width - INSET.left - INSET.right;
    const h = size.height - INSET.top - INSET.bottom;
    // On a 1/64 grid (the wheel's steps snap back onto powers of 1.12).
    const zoom = Math.min(1, Math.floor(Math.min(w / b.w, h / b.h) * 64) / 64);
    return {
      x: INSET.left + w / 2 - (b.x + b.w / 2) * zoom,
      y: INSET.top + h / 2 - (b.y + b.h / 2) * zoom,
      zoom,
      glide,
    };
  };
  const fit = () => {
    const v = framing(live(latest.current.graph), 520);
    if (v) setView(v);
  };

  // A preset: the old patch shrinks away, the new one pops in at its
  // tempo and is wired a moment later.
  const loads = useRef(0);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const load = (i: number) => {
    timers.current.forEach(clearTimeout);
    const tag = ++loads.current;
    const { graph, links } = instantiate(PRESETS[i], tag);
    const current = live(latest.current.graph);
    dispatch({ type: "delete", ids: current.map((n) => n.id) });
    setPreset(i);
    setBpm(PRESETS[i].bpm);
    const later = (ms: number, run: () => void) =>
      timers.current.push(setTimeout(run, ms));
    later(current.length ? 240 : 0, () => {
      dispatch({ type: "load", graph });
      const v = framing(Object.values(graph.nodes), 650);
      if (v) setView(v);
      later(WIRE_DELAY, () => dispatch({ type: "links", links }));
    });
  };
  useEffect(() => {
    if (size.width) load(0);
    // Once, when the window size is known.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [size.width > 0]);

  const arrangeAll = () => {
    const to = arrange(latest.current.graph);
    setGliding(true);
    // A frame later, so the cards hold `transition: { layout }` before
    // their rects change.
    setTimeout(() => {
      dispatch({ type: "move", to });
      const placed = live(latest.current.graph).map((n) => ({
        ...n,
        ...to[n.id],
      }));
      const v = framing(placed, 700);
      if (v) setView(v);
    }, 40);
    setTimeout(() => setGliding(false), 1400);
  };

  const add = (k: Kind, x: number, y: number) => {
    const { view } = latest.current;
    dispatch({
      type: "add",
      kind: k,
      x: (x - view.x) / view.zoom,
      y: (y - view.y) / view.zoom,
      select: true,
    });
  };
  // Mid-view, each click a little further along so they don't stack.
  const nudge = useRef(0);
  const addMid = (k: Kind) => {
    const { size } = latest.current;
    const n = nudge.current++ % 6;
    const cx = INSET.left + (size.width - INSET.left - INSET.right) / 2;
    const cy = INSET.top + (size.height - INSET.top - INSET.bottom) / 2;
    const { view } = latest.current;
    add(
      k,
      cx - (KINDS[k].width / 2) * view.zoom + n * 26,
      cy - (cardHeight(k) / 2) * view.zoom + n * 26,
    );
  };

  // Shortcuts. The Add menu takes the keyboard while it is open.
  useEffect(
    () =>
      bevy.on("keyDown", (e) => {
        if (e.repeat || menuOpen()) return;
        const { graph } = latest.current;
        const ctrl = e.ctrlKey || e.metaKey;
        if (ctrl && e.code === "KeyA")
          dispatch({ type: "select", ids: live(graph).map((n) => n.id) });
        else if (ctrl && e.code === "KeyD") dispatch({ type: "duplicate" });
        else if (ctrl) return;
        else if (e.code === "Space") setPlaying((p) => !p);
        else if (e.code === "KeyF") fit();
        else if (e.code === "KeyL") arrangeAll();
        else if (["Delete", "Backspace", "KeyX"].includes(e.code)) {
          dispatch({ type: "delete", ids: graph.selected });
        } else if (e.code === "Escape") dispatch({ type: "select", ids: [] });
      }),
    // Reads everything through `latest`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  // W A S D: Bevy reports what the held keys pan each frame (`view.rs`);
  // a short glide smooths over frames the message lands late.
  useEvent("view.pan", ({ x, y }) => {
    if (menuOpen()) return;
    setView((v) => ({ ...v, x: v.x + x, y: v.y + y, glide: 90 }));
  });

  // `--shoot` verbs (see shoot.rs).
  useDebug("preset", ([i]) => load(Number(i)));
  useDebug("play", () => setPlaying(true));
  useDebug("pause", () => setPlaying(false));
  useDebug("fit", fit);
  useDebug("arrange", arrangeAll);
  useDebug("add", ([k, x, y]) => add(k as Kind, Number(x), Number(y)));
  useDebug("view", ([x, y, z]) =>
    setView({ x: Number(x), y: Number(y), zoom: Number(z), glide: 400 }),
  );
  useDebug("select", (names) => {
    const ids = live(latest.current.graph)
      .filter((n) => names.includes(n.id.split("~")[0]))
      .map((n) => n.id);
    dispatch({ type: "select", ids });
  });

  return (
    <LiveContext.Provider value={ctx}>
      <node style={{ width: "100%", height: "100%" }}>
        <Canvas
          graph={graph}
          dispatch={dispatch}
          view={view}
          setView={setView}
          gliding={gliding}
          playing={playing}
        />
        <Palette onAdd={addMid} onDrop={(k, x, y) => add(k, x - 20, y - 18)} />
        <TopBar
          presets={PRESETS}
          preset={preset}
          onPreset={load}
          playing={playing}
          onPlay={() => setPlaying((p) => !p)}
          bpm={bpm}
          onBpm={setBpm}
          zoom={view.zoom}
          onArrange={arrangeAll}
          onFit={fit}
        />
        <Minimap
          graph={graph}
          view={view}
          size={size}
          onCenter={(x, y, glide) => {
            const { view, size } = latest.current;
            setView({
              ...view,
              x: size.width / 2 - x * view.zoom,
              y: size.height / 2 - y * view.zoom,
              glide,
            });
          }}
        />
        <Hints />
      </node>
    </LiveContext.Provider>
  );
}
