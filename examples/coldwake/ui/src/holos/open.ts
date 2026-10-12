import type { Game } from "../game/state";
import { hint } from "./hint";
import type { SurfaceName } from "./surfaces";

/** The fixtures' panels: always projected, live only while he stands within
 *  reach (Rust's `Holo::panel` in `holo/mod.rs`; a test there checks). */
export const PANELS: SurfaceName[] = [
  "store",
  "bench",
  "save",
  "elevator",
  "hack",
  "controls",
];

/** Which holograms are unfolded: the suit's vitals, the fixtures' panels
 *  and the floor indicator always; the rest while there is a reason — the
 *  lock until the door opens, the air in a vacuum, the ammo while he aims,
 *  the menu on Tab, the prompt while it has something to say. */
export function openHolos(g: Game): SurfaceName[] {
  const open: SurfaceName[] = ["spine", "stasis", "floor", ...PANELS];
  if (g.fixtures.door !== "open") open.push("lock");
  if (g.suit.vacuum) open.push("air");
  if (g.here.aiming) open.push("ammo");
  if (g.here.menu) open.push("menu");
  if (hint(g)) open.push("prompt");
  return open;
}

/** Whether hologram `name` is live: unfolded and, for a panel, within
 *  reach. A dead panel shows grey. */
export function isLive(g: Game, name: SurfaceName): boolean {
  if (PANELS.includes(name)) return g.here.near.includes(name);
  return openHolos(g).includes(name);
}
