import { useEffect, useReducer, useRef, useState } from "react";
import { bevy, emit, type Choice } from "./bevy";
import { catalogLoaded, def, setCatalog } from "./catalog";
import type { Commands } from "./graph/commands";
import { EditorContext, initialEditor, reduce } from "./graph/editor";
import { GraphView } from "./graph/GraphView";
import { evalGraph, nodeList } from "./graph/model";
import { useDebug, useEvent, useWindowSize } from "./hooks";
import { PopupLayer } from "./menus/popup";
import { TopBar } from "./menus/TopBar";
import { Preview2D } from "./panels/Preview2D";
import { Preview3D } from "./panels/Preview3D";
import { Sidebar } from "./panels/Sidebar";
import { PRESETS, blank } from "./presets/presets";
import { C, UI } from "./theme";

const GAP = 3;

/** Web builds evaluate on the main thread: start them at a lighter size. */
const DEFAULT_RESOLUTION = typeof window === "undefined" ? 512 : 256;

export function App() {
  const size = useWindowSize();
  const [loaded, setLoaded] = useState(catalogLoaded());
  const [state, dispatch] = useReducer(reduce, undefined, initialEditor);
  const [resolution, setResolution] = useState(DEFAULT_RESOLUTION);
  const [sidebar, setSidebar] = useState(true);
  const [pending, setPending] = useState<ReadonlySet<number>>(new Set());
  const [timings, setTimings] = useState<ReadonlyMap<number, number>>(
    new Map(),
  );
  const [viewOutput, setViewOutput] = useState<string | null>(null);
  const commands = useRef<Commands | null>(null);

  useEffect(() => {
    if (loaded) return;
    bevy.catalog().then((defs) => {
      setCatalog(defs);
      setLoaded(true);
    });
  }, [loaded]);

  const loadPreset = (id: string) => {
    const preset = PRESETS.find((p) => p.id === id);
    if (preset) dispatch({ type: "load", doc: preset.doc(), preset: id });
  };
  useEffect(() => {
    if (loaded && state.preset === null && !nodeList(state.doc).length)
      loadPreset(PRESETS[0].id);
  });
  useDebug("preset", ([id]) => loadPreset(id));
  useDebug("select", (ids) =>
    dispatch({
      type: "select",
      ids: ids.map(Number),
      mode: "set",
      active: Number(ids.at(-1)),
    }),
  );

  // Bevy evaluates what affects the result; moves and selection don't.
  const graph = loaded ? evalGraph(state.doc, resolution) : null;
  const graphKey = JSON.stringify(graph);
  const sent = useRef("");
  useEffect(() => {
    if (graph && graphKey !== sent.current) {
      sent.current = graphKey;
      emit("graph.set", graph);
    }
  });

  useEvent("graph.status", ({ pending, timings }) => {
    setPending(new Set(pending));
    setTimings(new Map(timings.map((t) => [t.id, t.ms])));
  });

  // The 2D view shows the active node, else the material's base color.
  const output = nodeList(state.doc).find((n) => n.kind === "output");
  const viewed =
    state.active !== null && state.doc.nodes[state.active]?.kind !== "reroute"
      ? state.doc.nodes[state.active]
      : output;
  const viewedDef = loaded && viewed ? def(viewed.kind) : null;
  const choices: Choice[] = viewedDef
    ? (viewedDef.outputs.length ? viewedDef.outputs : viewedDef.inputs).map(
        (s) => ({
          id: s.id,
          label: s.label,
        }),
      )
    : [];
  const shown =
    choices.find((c) => c.id === viewOutput)?.id ?? choices[0]?.id ?? null;
  useEffect(() => {
    emit("graph.view", { node: viewed?.id ?? null, output: shown });
  }, [viewed?.id, shown]);

  if (!size.width || !loaded) return null;

  const W = size.width;
  const H = size.height;
  const top = Math.round((H - UI.bar) * 0.38);
  const lower = H - UI.bar - top - GAP * 3;
  const viewWidth = Math.round(Math.min(W * 0.36, top * 1.25));
  const sideWidth = sidebar ? UI.sidebar : 0;
  const graphRect = {
    x: GAP,
    y: UI.bar + GAP * 2 + top,
    w: W - GAP * 2 - (sidebar ? sideWidth + GAP : 0),
    h: lower,
  };
  const totalMs = [...timings.values()].reduce((a, b) => a + b, 0);

  return (
    <EditorContext.Provider value={{ state, dispatch }}>
      <node
        style={{
          width: "100%",
          height: "100%",
          flexDirection: "column",
          backgroundColor: C.window,
        }}
      >
        <TopBar
          commands={() => commands.current}
          preset={state.preset}
          onPreset={loadPreset}
          onNew={() => dispatch({ type: "load", doc: blank(), preset: "new" })}
          resolution={resolution}
          onResolution={setResolution}
          pending={pending.size}
          totalMs={totalMs}
          sidebar={sidebar}
          onToggleSidebar={() => setSidebar(!sidebar)}
        />
        <node
          style={{
            flexDirection: "row",
            gap: GAP,
            padding: { horizontal: GAP, top: GAP },
          }}
        >
          <Preview3D width={W - GAP * 3 - viewWidth} height={top} />
          <Preview2D
            width={viewWidth}
            height={top}
            title={viewed ? (viewed.label ?? viewedDef!.label) : null}
            outputs={choices}
            output={shown}
            onOutput={setViewOutput}
          />
        </node>
        <node style={{ flexDirection: "row", gap: GAP, padding: GAP }}>
          <GraphView
            rect={graphRect}
            pending={pending}
            commands={commands}
            onToggleSidebar={() => setSidebar((s) => !s)}
          />
          {sidebar && (
            <Sidebar
              width={sideWidth}
              height={lower}
              timings={timings}
              pending={pending}
            />
          )}
        </node>
        <PopupLayer width={W} height={H} />
      </node>
    </EditorContext.Provider>
  );
}
