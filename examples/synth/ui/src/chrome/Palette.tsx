import { useRef, useState } from "react";
import type { PointerEventData } from "bevy-react";
import { CATEGORIES, type Category, type Kind, KINDS } from "../graph/catalog";
import { Icon } from "../graph/NodeCard";
import { C, alpha, glass } from "../theme";

export const PALETTE_W = 204;

/** The module palette: click a module to drop it mid-view, or drag it
 *  where it should go. */
export function Palette({
  onAdd,
  onDrop,
}: {
  onAdd: (k: Kind) => void;
  /** Dropped at a window point (logical px). */
  onDrop: (k: Kind, x: number, y: number) => void;
}) {
  const [ghost, setGhost] = useState<{
    kind: Kind;
    x: number;
    y: number;
  } | null>(null);
  const press = useRef<{
    kind: Kind;
    x: number;
    y: number;
    moved: boolean;
  } | null>(null);

  const down = (kind: Kind) => (e: PointerEventData) => {
    press.current = { kind, x: e.clientX, y: e.clientY, moved: false };
  };
  const move = (e: PointerEventData) => {
    const p = press.current;
    if (!p) return;
    if (!p.moved && Math.hypot(e.clientX - p.x, e.clientY - p.y) < 4) return;
    p.moved = true;
    setGhost({ kind: p.kind, x: e.clientX, y: e.clientY });
  };
  const up = (e: PointerEventData) => {
    const p = press.current;
    press.current = null;
    setGhost(null);
    if (!p) return;
    if (!p.moved) onAdd(p.kind);
    else if (e.clientX > PALETTE_W + 24) onDrop(p.kind, e.clientX, e.clientY);
  };

  return (
    <>
      <node
        style={{
          ...glass,
          positionType: "absolute",
          left: 14,
          top: 14,
          width: PALETTE_W,
          flexDirection: "column",
          padding: "12px 8px 8px",
        }}
      >
        <text
          style={{
            fontSize: 11,
            fontWeight: "semibold",
            letterSpacing: 1.4,
            color: C.dim,
            padding: { left: 6 },
            margin: { bottom: 4 },
          }}
        >
          MODULES
        </text>
        {(Object.keys(CATEGORIES) as Category[]).map((c) => (
          <node key={c} style={{ flexDirection: "column" }}>
            <node
              style={{
                flexDirection: "row",
                alignItems: "center",
                gap: 6,
                padding: { left: 6 },
                margin: { top: 8, bottom: 2 },
              }}
            >
              <node
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: 3,
                  backgroundColor: CATEGORIES[c].color,
                }}
              />
              <text style={{ fontSize: 10, letterSpacing: 1, color: C.dim }}>
                {CATEGORIES[c].label.toUpperCase()}
              </text>
            </node>
            {(Object.keys(KINDS) as Kind[])
              .filter((k) => KINDS[k].category === c)
              .map((k) => (
                <node
                  key={k}
                  onPointerDown={down(k)}
                  onPointerMove={move}
                  onPointerUp={up}
                  style={{
                    height: 32,
                    flexDirection: "row",
                    alignItems: "center",
                    gap: 9,
                    padding: { left: 6 },
                    borderRadius: 8,
                    cursor: "grab",
                    backgroundColor: "rgba(255, 255, 255, 0)",
                    transform: { translateX: 0 },
                    transition: {
                      backgroundColor: { duration: 120 },
                      transform: { stiffness: 420, damping: 26 },
                    },
                  }}
                  hoverStyle={{
                    backgroundColor: alpha(CATEGORIES[c].color, 0.14),
                    transform: { translateX: 3 },
                  }}
                >
                  <node
                    style={{
                      width: 24,
                      height: 24,
                      borderRadius: 7,
                      alignItems: "center",
                      justifyContent: "center",
                      backgroundColor: alpha(CATEGORIES[c].color, 0.14),
                    }}
                  >
                    <Icon
                      d={KINDS[k].icon}
                      color={CATEGORIES[c].color}
                      size={14}
                    />
                  </node>
                  <text style={{ fontSize: 12, color: C.text }}>
                    {KINDS[k].title}
                  </text>
                </node>
              ))}
          </node>
        ))}
      </node>
      {ghost && <Ghost {...ghost} />}
    </>
  );
}

/** What follows the pointer while a module is dragged out. */
function Ghost({ kind, x, y }: { kind: Kind; x: number; y: number }) {
  const def = KINDS[kind];
  const color = CATEGORIES[def.category].color;
  return (
    <node
      style={{
        positionType: "absolute",
        left: x - 18,
        top: y - 16,
        width: 168,
        height: 34,
        flexDirection: "row",
        alignItems: "center",
        gap: 9,
        padding: { left: 10 },
        borderRadius: 10,
        border: 1,
        borderColor: alpha(color, 0.7),
        backgroundColor: alpha("#151a26", 0.92),
        boxShadow: { color: alpha(color, 0.35), blurRadius: 18 },
        globalZIndex: 30,
        transform: { rotate: -3 },
      }}
    >
      <Icon d={def.icon} color={color} size={15} />
      <text style={{ fontSize: 12, fontWeight: "semibold", color: C.text }}>
        {def.title.toUpperCase()}
      </text>
    </node>
  );
}
