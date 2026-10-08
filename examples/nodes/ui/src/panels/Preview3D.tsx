import { useEffect, useRef, useState } from "react";
import { emit } from "../bevy";
import { useDebug } from "../hooks";
import type { IconName } from "../icons";
import { C, UI } from "../theme";
import { Pane, ToolButton } from "./Pane";

const MESHES: [string, IconName][] = [
  ["sphere", "sphere"],
  ["cube", "cube"],
  ["plane", "plane"],
  ["torus", "torus"],
];

/** The material on a mesh, rendered by Bevy into the `preview` target:
 *  drag to orbit, the wheel zooms. */
export function Preview3D({
  width,
  height,
}: {
  width: number;
  height: number;
}) {
  const [mesh, setMesh] = useState("sphere");
  const [spin, setSpin] = useState(true);
  const last = useRef({ x: 0, y: 0 });
  useEffect(() => emit("preview.set", { mesh, spin }), [mesh, spin]);
  useDebug("mesh", ([m]) => setMesh(m));

  return (
    <Pane
      title="Material Preview"
      width={width}
      height={height}
      tools={
        <>
          {MESHES.map(([id, icon]) => (
            <ToolButton
              key={id}
              icon={icon}
              active={mesh === id}
              onClick={() => setMesh(id)}
            />
          ))}
          <node style={{ width: 6 }} />
          <ToolButton
            icon="spin"
            active={spin}
            onClick={() => setSpin(!spin)}
          />
        </>
      }
    >
      <portal
        target="preview"
        style={{ flexGrow: 1, cursor: "grab" }}
        onPointerDown={(e) => {
          last.current = { x: e.clientX, y: e.clientY };
        }}
        onPointerMove={(e) => {
          emit("preview.orbit", {
            dx: e.clientX - last.current.x,
            dy: e.clientY - last.current.y,
            zoom: 0,
          });
          last.current = { x: e.clientX, y: e.clientY };
        }}
        onWheel={(e) => {
          const steps = e.deltaMode === "line" ? e.deltaY : e.deltaY / 40;
          emit("preview.orbit", { dx: 0, dy: 0, zoom: -steps });
        }}
      />
      <text
        style={{
          positionType: "absolute",
          left: 10,
          bottom: 6,
          fontSize: UI.small,
          color: C.textMuted,
        }}
      >
        Drag to orbit · wheel to zoom
      </text>
    </Pane>
  );
}
