import {
  call,
  fire,
  heal,
  hit,
  load,
  locate,
  nextDestination,
  reload,
  save,
  stasis,
  swap,
  vacuum,
} from "./game/act";
import { DESTINATIONS, type Destination } from "./game/state";
import { dispatch, getGame } from "./game/store";
import { useDebug, useEvent, useKeys } from "./hooks";
import { cue } from "./sound";

/** Open or fold the suit menu. */
function menu(open: boolean) {
  if (dispatch({ type: "menu", open })) cue(open ? "holoOpen" : "holoClose");
}

/** The suit's keys (Bevy reads the ones held every frame: W A S D, Shift
 *  and the mouse), the trigger Bevy pulls, and the `--shoot` verbs for
 *  everything he does outside a hologram. */
export function Controls() {
  useKeys(true, (e) => {
    const { here } = getGame();
    if (e.code === "Tab") return menu(!here.menu);
    // Reading the menu he stands still: a step folds it, and so does F
    // (as it lets go of any other screen).
    if (here.menu) {
      if (/^Key[WASDF]$/.test(e.code)) menu(false);
      return;
    }
    if (e.code === "KeyR") reload();
    else if (e.code === "KeyQ") swap();
    else if (e.code === "KeyM") heal();
    else if (e.code === "KeyC") stasis();
    else if (e.code === "KeyH") hit();
    else if (e.code === "KeyV") vacuum();
    else if (e.code === "KeyL") {
      if (e.shiftKey) nextDestination();
      else locate();
    }
  });
  useEvent("player.fire", () => fire());

  useDebug("menu", (arg) => menu(arg !== "off"));
  useDebug("hit", hit);
  useDebug("heal", heal);
  useDebug("stasis", stasis);
  useDebug("vacuum", (arg) => vacuum(arg ? arg === "on" : undefined));
  useDebug("fire", fire);
  useDebug("reload", reload);
  useDebug("swap", swap);
  useDebug("locator", (to) => {
    if (DESTINATIONS.includes(to as Destination)) locate(to as Destination);
    else locate();
  });
  useDebug("elevator", (floor) => call(Number(floor)));
  useDebug("save", save);
  useDebug("load", (id) => load(Number(id)));
  return null;
}
