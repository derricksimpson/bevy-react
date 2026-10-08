import { useState } from "react";
import type { Choice } from "../bevy";
import { EnumField } from "../fields/choice";
import { useDebug } from "../hooks";
import { C, UI } from "../theme";
import { Pane, ToolButton } from "./Pane";

/** The viewed output at full resolution (Bevy's `view` texture), tiled to
 *  check that it repeats seamlessly. */
export function Preview2D({
  width,
  height,
  title,
  outputs,
  output,
  onOutput,
}: {
  width: number;
  height: number;
  /** The viewed node's name. */
  title: string | null;
  outputs: Choice[];
  output: string | null;
  onOutput: (id: string) => void;
}) {
  const [tiles, setTiles] = useState(1);
  useDebug("tile", ([n]) => setTiles(Number(n)));
  // Whole-pixel cells, or the tiles' edges show seams.
  const cell = Math.floor(
    Math.max(0, Math.min(width - 20, height - UI.paneHeader - 20)) / tiles,
  );
  const side = cell * tiles;

  return (
    <Pane
      width={width}
      height={height}
      title={
        <text style={{ fontSize: UI.font, color: C.text, lineBreak: "noWrap" }}>
          {title ? `Image · ${title}` : "Image"}
        </text>
      }
      tools={
        <>
          {outputs.length > 1 && output && (
            <node style={{ width: 130 }}>
              <EnumField
                label="Output"
                value={output}
                options={outputs}
                z={1}
                onChange={onOutput}
              />
            </node>
          )}
          {[1, 2, 3].map((n) => (
            <ToolButton
              key={n}
              label={`${n}×`}
              active={tiles === n}
              onClick={() => setTiles(n)}
            />
          ))}
        </>
      }
    >
      <node
        style={{ flexGrow: 1, alignItems: "center", justifyContent: "center" }}
      >
        {title ? (
          <node style={{ width: side, height: side }}>
            {Array.from({ length: tiles * tiles }, (_, i) => {
              const [col, row] = [i % tiles, Math.floor(i / tiles)];
              // Each tile overlaps the next by a pixel: a quad's edge is
              // antialiased, and two touching edges show a hairline seam.
              return (
                <portal
                  key={i}
                  target="view"
                  style={{
                    positionType: "absolute",
                    left: col * cell,
                    top: row * cell,
                    width: cell + (col < tiles - 1 ? 1 : 0),
                    height: cell + (row < tiles - 1 ? 1 : 0),
                  }}
                />
              );
            })}
          </node>
        ) : (
          <text style={{ fontSize: UI.font, color: C.textMuted }}>
            Select a node to view it
          </text>
        )}
      </node>
    </Pane>
  );
}
