import { useEffect, useState, type ReactNode } from "react";
import {
  useSharedValue,
  withSequence,
  withTiming,
  type BevyStyle,
} from "bevy-react";
import { onGlitch } from "../game/act";
import { useGame } from "../game/store";
import { isLive, openHolos } from "./open";
import type { SurfaceName } from "./surfaces";

/** A folded hologram keeps its content this long (its fold-away runs
 *  0.3 s in `holo/mod.rs`), then drops it: nothing renders behind a closed
 *  hologram — no layout, no portal camera. */
const LINGER_MS = 400;

/** Whether hologram `name` is live (see `isLive`): what costs frames only
 *  while someone is near — a turntable's camera, a pulse — asks this. */
export function useLive(name: SurfaceName): boolean {
  return useGame((g) => isLive(g, name));
}

/** Whether the camera is locked on hologram `name`: he is using it. */
export function useHeld(name: SurfaceName): boolean {
  return useGame((g) => g.here.screen === name);
}

/** One hologram: a `<surface>` whose content wears the projector's noise,
 *  glitches when `glitch(name)` fires (a hit, a refused action) and, given
 *  a `page`, sweeps a scan line across every page change. Everything inside
 *  composites on the surface's own camera.
 *
 *  A panel nobody stands near is dead: Bevy draws it grey
 *  (`hologram.wesl`). Nothing points at it either: the only pointer is the
 *  cursor of the screen that holds the camera. */
export function Holo({
  name,
  page,
  seed = 0,
  style,
  children,
}: {
  name: SurfaceName;
  /** Changing it runs the `scanSwap` morph (tabs, pages). */
  page?: string | number;
  seed?: number;
  style?: BevyStyle;
  children: ReactNode;
}) {
  const open = useGame((g) => openHolos(g).includes(name));
  const [mounted, setMounted] = useState(open);
  useEffect(() => {
    if (open) return setMounted(true);
    const t = setTimeout(() => setMounted(false), LINGER_MS);
    return () => clearTimeout(t);
  }, [open]);
  const burst = useSharedValue(0);
  useEffect(
    () =>
      onGlitch((surface) => {
        if (surface !== name) return;
        burst.value = withSequence(
          withTiming(1, { duration: 0 }),
          withTiming(0, { duration: 480, easing: "easeOut" }),
        );
      }),
    [name, burst],
  );
  return (
    <surface target={name}>
      <node
        style={{
          width: "100%",
          height: "100%",
          ...style,
          filter: [
            { name: "holoNoise", params: { seed } },
            {
              name: "glitch",
              params: {
                intensity: { animated: burst, seed: 1 },
                seed: seed + 1,
              },
            },
          ],
          ...(page === undefined
            ? {}
            : {
                morphFilter: { key: page, name: "scanSwap" },
                transition: {
                  morphFilter: { duration: 380, easing: "easeInOut" },
                },
              }),
        }}
      >
        {mounted && children}
      </node>
    </surface>
  );
}
