import type { SurfaceName } from "../holos/surfaces";
import type { BenchTarget } from "./bench";
import { initialPack, type ItemId, type Pack, type WeaponId } from "./items";

/** Where the locator line can lead (Rust's `layout::destination`). */
export type Destination = "store" | "bench" | "save" | "elevator" | "bulkhead";
export const DESTINATIONS: Destination[] = [
  "store",
  "bench",
  "save",
  "elevator",
  "bulkhead",
];

/** One save at the save station: when, and a copy of the game to load. */
export type SaveSlot = {
  id: number;
  /** Station time, "04:12:33". */
  time: string;
  objective: number;
  game: Game;
};

/** The whole fake game. Pure data: `reduce.ts` changes it, `act.ts` turns
 *  player actions into changes plus their sounds and messages to Bevy. */
export type Game = {
  /** Where he stands and what his hands do: Bevy's side of the game
   *  (`holo.active`, `player.stance`) and the Tab key. A loaded save
   *  doesn't rewind it. */
  here: {
    /** The fixtures' panels he stands within reach of. */
    near: SurfaceName[];
    aiming: boolean;
    /** The app has taken the mouse. */
    mouse: boolean;
    /** The screen the camera is locked on: a panel he stands at, or the
     *  suit menu. Its cursor is the pointer. */
    screen: SurfaceName | null;
    /** The panel F would lock the camera on (he stands before it). */
    offer: SurfaceName | null;
    /** The suit menu is up. */
    menu: boolean;
    /** The first seconds: the prompt names the basic keys. */
    intro: boolean;
  };
  suit: {
    /** Vitals, 0..1 (the spine's segments). */
    health: number;
    /** The stasis module's charge, 0..1 (one use costs a quarter). */
    stasis: number;
    /** No air in the room: the air hologram counts down. */
    vacuum: boolean;
    /** Seconds of air left. */
    air: number;
  };
  arms: {
    weapon: WeaponId;
    owned: WeaponId[];
    /** Rounds in each weapon's clip. */
    clip: Record<WeaponId, number>;
  };
  pack: Pack;
  /** Powered circuit nodes per bench target. */
  bench: Record<BenchTarget, string[]>;
  story: {
    /** Something was bought (the first objective). */
    bought: boolean;
    read: string[];
  };
  fixtures: {
    door: "locked" | "unlocked" | "open";
    elevator: { floor: number; target: number | null };
    saves: SaveSlot[];
    locator: Destination;
  };
};

export function initialGame(): Game {
  return {
    here: {
      near: [],
      aiming: false,
      mouse: true,
      screen: null,
      offer: null,
      menu: false,
      intro: true,
    },
    suit: { health: 0.7, stasis: 0.75, vacuum: false, air: 60 },
    arms: {
      weapon: "cutter",
      owned: ["cutter"],
      clip: { cutter: 7, pulse: 25, rail: 4 },
    },
    pack: initialPack(),
    bench: { cutter: [], pulse: [], rail: [], rig: [] },
    story: { bought: false, read: [] },
    fixtures: {
      door: "locked",
      elevator: { floor: 2, target: null },
      saves: [],
      locator: "store",
    },
  };
}

/** Every change the game knows. */
export type Action =
  // Where he is (Bevy's reports) and the suit menu.
  | { type: "near"; surface: SurfaceName; on: boolean }
  | {
      type: "stance";
      aiming: boolean;
      mouse: boolean;
      screen: SurfaceName | null;
      offer: SurfaceName | null;
    }
  | { type: "menu"; open: boolean }
  | { type: "introOver" }
  // The suit.
  | { type: "hit" }
  | { type: "heal"; item: ItemId }
  | { type: "stasis" }
  | { type: "stasisRefill" }
  | { type: "vacuum"; on: boolean }
  | { type: "air"; dt: number }
  | { type: "airRefill" }
  // The weapons.
  | { type: "fire" }
  | { type: "reload" }
  | { type: "equip"; weapon: WeaponId }
  // The store and the safe.
  | { type: "buy"; item: ItemId }
  | { type: "sell"; slot: number }
  | { type: "stow"; slot: number }
  | { type: "retrieve"; index: number }
  // The bench.
  | { type: "install"; target: BenchTarget; node: string }
  // The story.
  | { type: "read"; log: string }
  // The fixtures.
  | { type: "unlock" }
  | { type: "opened" }
  | { type: "save"; time: string }
  | { type: "load"; id: number }
  | { type: "call"; floor: number }
  | { type: "arrived"; floor: number }
  | { type: "locator"; to: Destination };
