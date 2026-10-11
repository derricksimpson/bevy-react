import { useEffect, useRef, useState } from "react";
import { bevy } from "./bevy";
import { LAST_BEAT, SPOTS } from "./beats";
import { fire } from "./game/act";
import { OBJECTIVES } from "./game/lore";
import { objective } from "./game/reduce";
import { dispatch, getGame, useGame } from "./game/store";
import { useDebug, useEvent, useKeys } from "./hooks";
import { Air } from "./holos/Air";
import { Ammo } from "./holos/Ammo";
import { Bench } from "./holos/Bench";
import { Elevator } from "./holos/Elevator";
import { Floor } from "./holos/Floor";
import { Hack } from "./holos/Hack";
import { Lock } from "./holos/Lock";
import { Menu } from "./holos/Menu";
import { Save } from "./holos/Save";
import { Spine } from "./holos/Spine";
import { Stasis } from "./holos/Stasis";
import { Store } from "./holos/Store";
import { Title } from "./holos/Title";
import { openHolos } from "./holos/open";
import type { SurfaceName } from "./holos/surfaces";
import { Controls } from "./controls";
import { BeatStrip } from "./hud/BeatStrip";
import { Dip } from "./hud/Dip";
import { cue } from "./sound";

/** The holograms that big they unfold with a sound. */
const LOUD: SurfaceName[] = ["menu", "store", "bench", "hack", "title", "air"];

/** Move the tour to `beat`: Bevy films it, and a beat at another spot cuts
 *  through the static dip. */
function useTour() {
  const [dip, setDip] = useState(0);
  const go = (next: number) => {
    const beat = Math.max(0, Math.min(LAST_BEAT, next));
    const from = getGame().beat;
    if (beat === from) return;
    dispatch({ type: "beat", beat });
    bevy.tour.beat({ beat });
    if (SPOTS[beat] !== SPOTS[from]) {
      setDip((n) => n + 1);
      cue("static");
    }
  };
  return { dip, go };
}

export function App() {
  const game = useGame();
  const { dip, go } = useTour();
  const [dipping, setDipping] = useState(false);

  // The dip stays mounted for its run, then leaves.
  useEffect(() => {
    if (dip === 0) return;
    setDipping(true);
    const t = setTimeout(() => setDipping(false), 650);
    return () => clearTimeout(t);
  }, [dip]);

  // Tell Bevy which holograms are unfolded (only the changes).
  const shown = useRef(new Set<SurfaceName>());
  const open = openHolos(game).join(",");
  useEffect(() => {
    const next = new Set(open.split(",") as SurfaceName[]);
    for (const surface of next) {
      if (shown.current.has(surface)) continue;
      bevy.holo.open({ surface, open: true });
      if (LOUD.includes(surface)) cue("holoOpen");
    }
    for (const surface of shown.current) {
      if (next.has(surface)) continue;
      bevy.holo.open({ surface, open: false });
      if (LOUD.includes(surface)) cue("holoClose");
    }
    shown.current = next;
  }, [open]);

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

  // ← → walk the strip, 1–8 jump (not on the title: BEGIN leaves it).
  useKeys(game.beat > 0, (e) => {
    if (e.code === "ArrowLeft") go(game.beat - 1);
    else if (e.code === "ArrowRight") go(game.beat + 1);
    else if (/^Digit[1-8]$/.test(e.code)) go(Number(e.code.slice(5)));
  });
  useDebug("beat", (n) => go(Number(n)));

  return (
    <node style={{ width: "100%", height: "100%" }}>
      <Title onBegin={() => go(1)} />
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

      {/* Aiming: a click anywhere fires (nothing else is clickable here). */}
      {game.beat === 2 && (
        <node
          style={{
            positionType: "absolute",
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
          }}
          onPointerDown={() => fire()}
        />
      )}
      {game.beat > 0 && <BeatStrip beat={game.beat} onBeat={go} />}
      {dipping && <Dip key={dip} />}
    </node>
  );
}
