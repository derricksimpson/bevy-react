import type { Game } from "../game/state";
import type { SurfaceName } from "./surfaces";

/** One line of the prompt: what to press, what it does. */
export type Hint = { id: string; keys: [key: string, what: string][] };

/** What F would have him use, by panel. */
const USE: Partial<Record<SurfaceName, string>> = {
  store: "use the store",
  bench: "use the bench",
  save: "use the save station",
  elevator: "use the lift panel",
  hack: "bypass the lock",
  controls: "read the terminal",
};

/** What the prompt beside his arm says now, if anything: how to take the
 *  mouse, that F would lock the camera on the panel he stands before, the
 *  basic keys for the first seconds. Silent while a screen holds the camera
 *  or the gun is up (they are in front of it). */
export function hint(g: Game): Hint | null {
  const { mouse, screen, offer, aiming, intro } = g.here;
  if (!mouse) return { id: "click", keys: [["CLICK", "take the mouse"]] };
  if (screen || aiming) return null;
  if (offer) return { id: offer, keys: [["F", USE[offer] ?? "use it"]] };
  if (intro)
    return {
      id: "intro",
      keys: [
        ["WASD", "move"],
        ["RMB", "aim"],
        ["TAB", "suit menu"],
      ],
    };
  return null;
}
