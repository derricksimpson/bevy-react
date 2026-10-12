import { bevy } from "../bevy";
import type { SurfaceName } from "../holos/surfaces";
import { cue } from "../sound";
import { ITEMS, WEAPONS, type ItemId, type WeaponId } from "./items";
import type { BenchTarget } from "./bench";
import { DESTINATIONS, type Destination } from "./state";
import { OBJECTIVES } from "./lore";
import { HEALS, objective } from "./reduce";
import { dispatch, getGame } from "./store";

/** The player's verbs: each changes the game and plays its part in the
 *  world (a sound, a message to Bevy) or, refused, glitches the hologram
 *  that refused it. Keys, clicks and `--shoot` verbs all come through
 *  here. */

const glitchers = new Set<(surface: SurfaceName) => void>();

/** Glitch hologram `surface` for a moment (`Holo` listens). */
export function glitch(surface: SurfaceName) {
  glitchers.forEach((run) => run(surface));
}

/** Subscribe to glitches (see `Holo`); returns the unsubscribe. */
export function onGlitch(run: (surface: SurfaceName) => void) {
  glitchers.add(run);
  return () => {
    glitchers.delete(run);
  };
}

/** A refused action: the buzz and the glitch. */
function deny(surface: SurfaceName) {
  cue("deny");
  glitch(surface);
}

export function hit() {
  dispatch({ type: "hit" });
  cue("hit");
  bevy.suit.hit(null);
  glitch("spine");
  glitch("stasis");
}

/** Use the largest medpack that isn't wasted (else the smallest there). */
export function heal() {
  const { pack, suit } = getGame();
  const missing = 1 - suit.health;
  const have = (["medpack-s", "medpack-m", "medpack-l"] as ItemId[]).filter(
    (id) => pack.slots.some((s) => s?.item === id),
  );
  const fit = [...have]
    .reverse()
    .find((id) => (HEALS[id] ?? 1) <= missing + 0.01);
  const item = fit ?? have[0];
  if (item && dispatch({ type: "heal", item })) {
    cue("heal");
    bevy.suit.heal(null);
  } else {
    deny("spine");
  }
}

/** Use an inventory slot's item (the suit menu's USE). */
export function use(slot: number) {
  const item = getGame().pack.slots[slot]?.item;
  if (!item) return;
  const kind = ITEMS[item].kind;
  if (kind === "health") {
    if (dispatch({ type: "heal", item })) {
      cue("heal");
      bevy.suit.heal(null);
      return;
    }
  } else if (kind === "stasis") {
    if (dispatch({ type: "stasisRefill" })) return cue("stasis");
  } else if (kind === "air") {
    if (dispatch({ type: "airRefill" })) return cue("select");
  } else if (kind === "ammo") {
    const weapon = WEAPONS.find((w) => item === `${w}-ammo`);
    if (weapon === getGame().arms.weapon) return reload();
  }
  deny("menu");
}

export function stasis() {
  if (dispatch({ type: "stasis" })) {
    cue("stasis");
    bevy.suit.stasis(null);
  } else {
    deny("stasis");
  }
}

/** Vent the room (or let the air back). Bevy follows the game's vacuum
 *  (`App.tsx`), so a loaded save sets it too. */
export function vacuum(on = !getGame().suit.vacuum) {
  dispatch({ type: "vacuum", on });
}

export function fire() {
  if (dispatch({ type: "fire" })) {
    cue("fire");
    bevy.weapon.fire(null);
  } else {
    cue("dryFire");
    glitch("ammo");
  }
}

export function reload() {
  if (dispatch({ type: "reload" })) {
    cue("reload");
    bevy.weapon.reload(null);
  } else {
    deny("ammo");
  }
}

/** Draw `weapon` (Bevy follows the game's weapon, `App.tsx`). */
export function equip(weapon: WeaponId) {
  if (dispatch({ type: "equip", weapon })) cue("equip");
}

/** The next weapon he owns. */
export function swap() {
  const { owned, weapon } = getGame().arms;
  const order = WEAPONS.filter((w) => owned.includes(w));
  equip(order[(order.indexOf(weapon) + 1) % order.length]);
}

export function buy(item: ItemId) {
  if (dispatch({ type: "buy", item })) cue("buy");
  else deny("store");
}

export function sell(slot: number) {
  if (dispatch({ type: "sell", slot })) cue("buy");
  else deny("store");
}

export function stow(slot: number) {
  if (dispatch({ type: "stow", slot })) cue("select");
  else deny("store");
}

export function retrieve(index: number) {
  if (dispatch({ type: "retrieve", index })) cue("select");
  else deny("store");
}

export function install(target: BenchTarget, node: string) {
  if (dispatch({ type: "install", target, node })) cue("install");
  else deny("bench");
}

export function read(log: string) {
  dispatch({ type: "read", log });
}

/** The bypass solved the lock: the bulkhead opens. */
export function unlock() {
  if (dispatch({ type: "unlock" })) {
    cue("unlock");
    bevy.door.unlock(null);
  }
}

/** A failed lock attempt. */
export function shock() {
  cue("shock");
  bevy.hack.shock(null);
  glitch("hack");
}

export function save() {
  const now = new Date();
  const time = [now.getHours(), now.getMinutes(), now.getSeconds()]
    .map((n) => String(n).padStart(2, "0"))
    .join(":");
  dispatch({ type: "save", time });
  cue("save");
}

export function load(id: number) {
  if (dispatch({ type: "load", id })) cue("save");
}

export function call(floor: number) {
  if (dispatch({ type: "call", floor })) {
    cue("select");
    bevy.elevator.call({ floor });
  } else {
    deny("elevator");
  }
}

/** Trace the locator line: to `to`, else to where the current objective
 *  leads. */
export function locate(to: Destination = OBJECTIVES[objective(getGame())].to) {
  dispatch({ type: "locator", to });
  cue("locator");
  bevy.locator.show({ to });
}

/** Pick the next destination and trace to it. */
export function nextDestination() {
  const at = DESTINATIONS.indexOf(getGame().fixtures.locator);
  locate(DESTINATIONS[(at + 1) % DESTINATIONS.length]);
}
