import { bevy } from "./bevy";

/** The cues Bevy synthesizes (`sound/`). */
export type Cue =
  // Holograms and their UI.
  | "holoOpen"
  | "holoClose"
  | "tick"
  | "select"
  | "deny"
  | "tab"
  | "buy"
  | "install"
  // The suit.
  | "hit"
  | "heal"
  | "stasis"
  | "lowAir"
  // The weapons.
  | "fire"
  | "dryFire"
  | "reload"
  | "equip"
  // The fixtures.
  | "unlock"
  | "hackTick"
  | "hackLock"
  | "shock"
  | "save"
  | "locator"
  | "log";

/** Play a cue. */
export function cue(name: Cue) {
  bevy.sound.cue({ name });
}
