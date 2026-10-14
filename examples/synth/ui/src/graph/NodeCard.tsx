import { memo, useEffect, useRef } from "react";
import {
  interpolate,
  useSharedValue,
  withDelay,
  withSequence,
  withSpring,
  withTiming,
  type BevyStyle,
  type Gradient,
  type PointerEventData,
} from "bevy-react";
import { BODIES } from "../bodies";
import { C, SIGNAL, alpha, r } from "../theme";
import { Led, useLive } from "../widgets";
import {
  BODY_PAD,
  CATEGORIES,
  HEADER,
  ROW,
  ROWS_PAD,
  cardHeight,
  kind,
} from "./catalog";
import { type GraphNode, type Side, type SocketRef, socketName } from "./model";

/** What the canvas does with a card's gestures. One stable object, so the
 *  memoized cards never re-render for a new callback. */
export type CardApi = {
  press(id: string, e: PointerEventData): void;
  drag(e: PointerEventData): void;
  release(e: PointerEventData): void;
  socketDown(s: SocketRef, e: PointerEventData): void;
  socketUp(e: PointerEventData): void;
  socketHover(s: SocketRef, on: boolean): void;
  param(id: string, key: string, value: number): void;
  purge(id: string): void;
};

/** How a socket looks while a wire is being pulled. */
export type Plug = "idle" | "open" | "shut" | "target";

const RADIUS = 12;

export const NodeCard = memo(function NodeCard({
  node,
  selected,
  lifted,
  gliding,
  wired,
  plug,
  api,
}: {
  node: GraphNode;
  selected: boolean;
  /** Being dragged. */
  lifted: boolean;
  /** Gliding to an arranged position. */
  gliding: boolean;
  /** Sockets with a wire, as `"i0 o1"`. */
  wired: string;
  /** Per socket while a wire is pulled (`"i0"` → `"open"`); `null` when not. */
  plug: Record<string, Plug> | null;
  api: CardApi;
}) {
  const def = kind(node.kind);
  const color = CATEGORIES[def.category].color;
  const { spin, steps } = useLive();

  // Pop in (staggered by `enter`), shrink away when deleted.
  const appear = useSharedValue(0);
  useEffect(() => {
    appear.value = withDelay(
      node.enter,
      withSpring(1, { stiffness: 240, damping: 16 }),
    );
  }, [appear, node.enter]);
  useEffect(() => {
    if (node.leaving) {
      appear.value = withTiming(0, {
        duration: 180,
        easing: "easeIn",
        onComplete: () => api.purge(node.id),
      });
    }
  }, [appear, node.leaving, node.id, api]);

  // Lifted while dragged, leaning into the motion and springing back.
  const lift = useSharedValue(0);
  const tilt = useSharedValue(0);
  useEffect(() => {
    lift.value = withSpring(lifted ? 1 : 0, { stiffness: 380, damping: 22 });
  }, [lift, lifted]);
  const lastX = useRef(node.x);
  useEffect(() => {
    const dx = node.x - lastX.current;
    lastX.current = node.x;
    if (!lifted || dx === 0) return;
    const lean = Math.max(-7, Math.min(7, dx * 0.35));
    tilt.value = withSequence(
      withTiming(lean, { duration: 90, easing: "easeOut" }),
      withSpring(0, { stiffness: 150, damping: 8 }),
    );
  }, [tilt, node.x, lifted]);

  const ring: Gradient = {
    type: "conic",
    start: { animated: spin },
    stops: [
      { color },
      { color: alpha(color, 0.12), angle: 90 },
      { color: "#ffffff", angle: 180 },
      { color: alpha(color, 0.12), angle: 270 },
      { color, angle: 360 },
    ],
  };
  const drop = {
    color: "rgba(0, 0, 0, 0.55)",
    yOffset: r(lifted ? 22 : 10),
    blurRadius: r(lifted ? 40 : 24),
  };
  const rows = Math.max(def.inputs.length, def.outputs.length);
  const Body = BODIES[node.kind];
  const set = (key: string, value: number) => api.param(node.id, key, value);

  return (
    <node
      style={{
        positionType: "absolute",
        left: r(node.x),
        top: r(node.y),
        width: r(def.width),
        height: r(cardHeight(node.kind)),
        zIndex: node.z,
        transform: {
          scale: { animated: interpolate(appear, [0, 1, 2], [0.02, 1, 2]) },
        },
        transition: gliding
          ? { layout: { stiffness: 110, damping: 16 } }
          : undefined,
      }}
    >
      <node
        onPointerDown={(e) => api.press(node.id, e)}
        onPointerMove={(e) => api.drag(e)}
        onPointerUp={(e) => api.release(e)}
        style={{
          flexGrow: 1,
          flexDirection: "column",
          borderRadius: r(RADIUS),
          border: r(1.5),
          borderColor: selected ? "transparent" : C.edge,
          borderGradient: selected ? ring : undefined,
          backgroundGradient: {
            type: "linear",
            angle: 180,
            // See-through: the wires run under the cards.
            stops: [
              { color: alpha("#181d2b", 0.72) },
              { color: alpha("#11151f", 0.8) },
            ],
          },
          boxShadow: selected
            ? [
                {
                  color: alpha(color, 0.32),
                  blurRadius: r(26),
                  spreadRadius: r(1),
                },
                drop,
              ]
            : drop,
          transform: {
            scale: { animated: interpolate(lift, [0, 1, 2], [1, 1.035, 1.07]) },
            rotate: { animated: tilt },
          },
          cursor: lifted ? "grabbing" : "grab",
        }}
      >
        {/* The cursor light (`spotlight.rs`, all in Bevy): the border catches
            it, the surface takes a soft wash. First: under the content. */}
        <node
          style={{
            positionType: "absolute",
            left: r(-1.5),
            top: r(-1.5),
            right: r(-1.5),
            bottom: r(-1.5),
            borderRadius: r(RADIUS),
            spotlight: { color, edge: 1.5, wash: 0.12 },
          }}
        />
        <node
          style={{
            height: r(HEADER),
            flexDirection: "row",
            alignItems: "center",
            gap: r(8),
            padding: { horizontal: r(12) },
            borderRadius: `${r(RADIUS - 1.5)} ${r(RADIUS - 1.5)} 0 0`,
            backgroundGradient: {
              type: "linear",
              angle: 90,
              stops: [
                { color: alpha(color, 0.3) },
                { color: alpha(color, 0.03) },
              ],
            },
          }}
        >
          <Icon d={def.icon} color={color} size={r(16)} />
          <text
            style={{
              flexGrow: 1,
              fontSize: r(12),
              fontWeight: "semibold",
              letterSpacing: r(0.5),
              color: C.text,
            }}
          >
            {def.title.toUpperCase()}
          </text>
          <Led
            color={color}
            level={(j) => (wired && j % 2 === 0 ? 1 : 0)}
            size={6}
          />
        </node>
        <node style={{ height: r(1), backgroundColor: C.edge }} />
        {rows > 0 && (
          <node
            style={{
              flexDirection: "column",
              padding: { vertical: r(ROWS_PAD) },
            }}
          >
            {Array.from({ length: rows }, (_, i) => (
              <node
                key={i}
                style={{
                  height: r(ROW),
                  flexDirection: "row",
                  alignItems: "center",
                  justifyContent: "spaceBetween",
                }}
              >
                {[def.inputs[i], def.outputs[i]].map((p, k) => {
                  const side: Side = k === 0 ? "i" : "o";
                  return p ? (
                    <Socket
                      key={side}
                      at={{ node: node.id, side, port: i }}
                      name={p.name}
                      color={SIGNAL[p.signal]}
                      wired={wired.includes(`${side}${i}`)}
                      plug={plug?.[`${side}${i}`] ?? "idle"}
                      api={api}
                    />
                  ) : (
                    <node key={side} />
                  );
                })}
              </node>
            ))}
          </node>
        )}
        <node
          style={{
            height: r(def.body),
            flexDirection: "column",
            padding: { horizontal: r(BODY_PAD) },
          }}
        >
          <Body node={node} set={set} color={color} steps={steps} />
        </node>
      </node>
    </node>
  );
});

function Socket({
  at,
  name,
  color,
  wired,
  plug,
  api,
}: {
  at: SocketRef;
  name: string;
  color: string;
  wired: boolean;
  plug: Plug;
  api: CardApi;
}) {
  const input = at.side === "i";
  const { spin } = useLive();
  const scale = { idle: 1, open: 1.1, shut: 0.8, target: 1.6 }[plug];
  // Sockets that would take the pulled wire breathe, all in step.
  const breath = interpolate(
    spin,
    [0, 45, 90, 135, 180, 225, 270, 315, 360],
    [1, 1.3, 1, 1.3, 1, 1.3, 1, 1.3, 1],
  );
  const dim = plug === "shut" ? 0.25 : 1;
  const hit: BevyStyle = {
    positionType: "absolute",
    left: input ? r(-12.75) : undefined,
    right: input ? undefined : r(-12.75),
    top: r(0),
    width: r(24),
    height: r(24),
    alignItems: "center",
    justifyContent: "center",
    transform: { scale },
    transition: { transform: { stiffness: 420, damping: 20 } },
    cursor: "crosshair",
  };
  return (
    <node
      style={{
        flexDirection: input ? "row" : "rowReverse",
        alignItems: "center",
        height: r(ROW),
        padding: { left: r(input ? 14 : 0), right: r(input ? 0 : 14) },
      }}
    >
      <node
        name={socketName(at)}
        onPointerDown={(e) => api.socketDown(at, e)}
        onPointerUp={(e) => api.socketUp(e)}
        onPointerEnter={() => api.socketHover(at, true)}
        onPointerLeave={() => api.socketHover(at, false)}
        style={hit}
        hoverStyle={
          plug === "idle" ? { transform: { scale: 1.35 } } : undefined
        }
      >
        <node
          style={{
            width: r(12),
            height: r(12),
            borderRadius: "50%",
            border: r(2),
            borderColor: color,
            backgroundColor: wired || plug === "target" ? color : C.cardDeep,
            boxShadow:
              plug === "open" || plug === "target"
                ? {
                    color: alpha(color, 0.75),
                    blurRadius: r(10),
                    spreadRadius: r(1),
                  }
                : undefined,
            opacity: dim,
            transform: { scale: plug === "open" ? { animated: breath } : 1 },
            transition: { opacity: { duration: 160 } },
          }}
        />
      </node>
      <text
        style={{
          fontSize: r(10),
          color: plug === "shut" ? C.faint : C.dim,
          letterSpacing: r(0.3),
        }}
      >
        {name}
      </text>
    </node>
  );
}

/** A catalog icon; `size` is a length (`r(16)` inside the graph). */
export function Icon({
  d,
  color,
  size,
}: {
  d: string;
  color: string;
  size: number | string;
}) {
  return (
    <svg viewBox="0 0 24 24" style={{ width: size, height: size }}>
      <path
        d={d}
        fill="none"
        stroke={color}
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
