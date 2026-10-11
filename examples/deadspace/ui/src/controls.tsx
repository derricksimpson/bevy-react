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
import { useGame } from "./game/store";
import { useDebug, useKeys } from "./hooks";

/** The keys of the beats whose holograms only show (the suit, aiming, the
 *  locator), and the `--shoot` verbs for everything the player does there
 *  and at the fixtures. */
export function Controls() {
  const beat = useGame((g) => g.beat);

  useKeys(beat === 1, (e) => {
    if (e.code === "KeyH") hit();
    else if (e.code === "KeyM") heal();
    else if (e.code === "KeyS") stasis();
    else if (e.code === "KeyV") vacuum();
  });
  useKeys(beat === 2, (e) => {
    if (e.code === "KeyF") fire();
    else if (e.code === "KeyR") reload();
    else if (e.code === "KeyQ") swap();
  });
  useKeys(beat === 8, (e) => {
    if (e.code !== "KeyL") return;
    if (e.shiftKey) nextDestination();
    else locate();
  });

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
