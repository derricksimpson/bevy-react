import type { Game } from "../game/state";
import type { SurfaceName } from "./surfaces";

/** The beat each beat-owned hologram belongs to (index = beat). */
const BEAT_HOLOS: (SurfaceName | null)[] = [
  "title",
  null,
  "ammo",
  "menu",
  "store",
  "bench",
  null,
  "hack",
  null,
];

/** Which holograms are unfolded: the suit's and the fixtures' screens are
 *  always on, the rest belong to their beat (or, the air, to a vacuum). */
export function openHolos(g: Game): SurfaceName[] {
  const open: SurfaceName[] = ["spine", "stasis", "save", "elevator", "floor"];
  if (g.fixtures.door !== "open") open.push("lock");
  if (g.suit.vacuum) open.push("air");
  const own = BEAT_HOLOS[g.beat];
  if (own) open.push(own);
  return open;
}
