/** The tour, beat by beat: what the strip shows and which keys each beat
 *  answers. A beat's `spot` is where the camera films it (Rust's `SHOTS` in
 *  `tour.rs`): moving between beats of one spot glides, to another spot
 *  cuts through the static dip. */
export const BEATS = [
  { name: "TITLE", keys: [["ENTER", "begin"]] },
  {
    name: "SUIT",
    keys: [
      ["H", "take a hit"],
      ["M", "medpack"],
      ["S", "stasis"],
      ["V", "vacuum"],
    ],
  },
  {
    name: "WEAPON",
    keys: [
      ["F / LMB", "fire"],
      ["R", "reload"],
      ["Q", "swap"],
    ],
  },
  {
    name: "SUIT MENU",
    keys: [
      ["TAB", "next tab"],
      ["CLICK", "select"],
      ["WHEEL", "scroll"],
    ],
  },
  {
    name: "STORE",
    keys: [
      ["CLICK", "buy · sell · store"],
      ["WHEEL", "scroll"],
    ],
  },
  { name: "BENCH", keys: [["CLICK", "install a node"]] },
  { name: "DOORS", keys: [["CLICK", "save · load · call"]] },
  {
    name: "HACK",
    keys: [
      ["MOUSE / A D", "turn"],
      ["E / CLICK", "try"],
    ],
  },
  {
    name: "LOCATOR",
    keys: [
      ["L", "trace"],
      ["SHIFT+L", "destination"],
    ],
  },
] as const;

/** Where each beat is filmed (index = beat). Must match `SHOTS` in
 *  `tour.rs` (a Rust test checks). */
export const SPOTS = [0, 1, 1, 1, 2, 3, 4, 4, 1];

export const LAST_BEAT = BEATS.length - 1;
