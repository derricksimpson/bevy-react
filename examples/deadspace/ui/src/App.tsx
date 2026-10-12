import { useEffect, useRef } from "react";
import { bevy } from "./bevy";
import { OBJECTIVES } from "./game/lore";
import { objective } from "./game/reduce";
import { dispatch, useGame } from "./game/store";
import { useEvent } from "./hooks";
import { Air } from "./holos/Air";
import { Ammo } from "./holos/Ammo";
import { Bench } from "./holos/Bench";
import { Elevator } from "./holos/Elevator";
import { Floor } from "./holos/Floor";
import { Hack } from "./holos/Hack";
import { Lock } from "./holos/Lock";
import { Menu } from "./holos/Menu";
import { Prompt } from "./holos/Prompt";
import { Save } from "./holos/Save";
import { Spine } from "./holos/Spine";
import { Stasis } from "./holos/Stasis";
import { Store } from "./holos/Store";
import { Terminal } from "./holos/Terminal";
import { openHolos } from "./holos/open";
import type { SurfaceName } from "./holos/surfaces";
import { Controls } from "./controls";
import { cue } from "./sound";

/** The prompt names the basic keys this long. */
const INTRO_MS = 20_000;

export function App() {
  const game = useGame();

  // Bevy says where he is and what his hands do; ask for all of it once
  // (this tree may have mounted after Bevy last said it).
  useEvent("holo.active", ({ surface, active }) =>
    dispatch({ type: "near", surface: surface as SurfaceName, on: active }),
  );
  useEvent("player.stance", ({ aiming, mouse, screen, offer }) =>
    dispatch({
      type: "stance",
      aiming,
      mouse,
      screen: screen as SurfaceName | null,
      offer: offer as SurfaceName | null,
    }),
  );
  useEffect(() => bevy.player.sync(null), []);
  useEffect(() => {
    const t = setTimeout(() => dispatch({ type: "introOver" }), INTRO_MS);
    return () => clearTimeout(t);
  }, []);

  // Tell Bevy which holograms are unfolded (only the changes).
  const shown = useRef(new Set<SurfaceName>());
  const open = openHolos(game).join(",");
  useEffect(() => {
    const next = new Set(open.split(",") as SurfaceName[]);
    for (const surface of next)
      if (!shown.current.has(surface)) bevy.holo.open({ surface, open: true });
    for (const surface of shown.current)
      if (!next.has(surface)) bevy.holo.open({ surface, open: false });
    shown.current = next;
  }, [open]);

  // The air readout comes on, and goes, with a sound. (The fixtures'
  // panels wake and sleep silently; the suit menu's sound is its key's,
  // `controls.tsx`.)
  const vented = useRef(game.suit.vacuum);
  useEffect(() => {
    if (vented.current === game.suit.vacuum) return;
    vented.current = game.suit.vacuum;
    cue(game.suit.vacuum ? "holoOpen" : "holoClose");
  }, [game.suit.vacuum]);

  // The weapon in his hands and the room's air follow the game (a load can
  // change either).
  useEffect(() => {
    bevy.weapon.equip({ weapon: game.arms.weapon });
  }, [game.arms.weapon]);
  useEffect(() => {
    bevy.suit.vacuum({ on: game.suit.vacuum });
  }, [game.suit.vacuum]);

  // The map marks where the current objective leads.
  const to = OBJECTIVES[objective(game)].to;
  useEffect(() => bevy.map.mark({ to }), [to]);

  // The tanks drain while the room has no air.
  useEffect(() => {
    if (!game.suit.vacuum) return;
    const t = setInterval(() => dispatch({ type: "air", dt: 0.25 }), 250);
    return () => clearInterval(t);
  }, [game.suit.vacuum]);

  // The suit beeps while the tanks run low.
  const lowAir = game.suit.vacuum && game.suit.air < 15;
  useEffect(() => {
    if (!lowAir) return;
    cue("lowAir");
    const t = setInterval(() => cue("lowAir"), 1800);
    return () => clearInterval(t);
  }, [lowAir]);

  useEvent("door.opened", () => dispatch({ type: "opened" }));
  useEvent("elevator.arrived", ({ floor }) =>
    dispatch({ type: "arrived", floor }),
  );

  // Nothing is drawn on the screen itself: every readout is a surface out
  // in the room.
  return (
    <>
      <Terminal />
      <Prompt />
      <Spine />
      <Stasis />
      <Air />
      <Ammo />
      <Menu />
      <Store />
      <Bench />
      <Lock />
      <Save />
      <Elevator />
      <Floor />
      <Hack />
      <Controls />
    </>
  );
}
