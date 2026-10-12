/** The controls, as the terminal in the alcove lists them. */
export const CONTROLS: {
  title: string;
  keys: [key: string, what: string][];
}[] = [
  {
    title: "MOVE",
    keys: [
      ["W A S D", "walk"],
      ["SHIFT", "hurry"],
      ["MOUSE", "look"],
      ["ESC", "free the mouse"],
    ],
  },
  {
    title: "SCREENS",
    keys: [
      ["F", "use the one he faces"],
      ["MOUSE", "cursor"],
      ["LMB", "press"],
      ["WHEEL", "scroll"],
      ["F · W A S D", "leave it"],
    ],
  },
  {
    title: "WEAPON",
    keys: [
      ["RMB", "aim"],
      ["LMB", "fire"],
      ["R", "reload"],
      ["Q", "next weapon"],
    ],
  },
  {
    title: "TEST RIG",
    keys: [
      ["H", "take a hit"],
      ["V", "vent the room"],
    ],
  },
  {
    title: "SUIT",
    keys: [
      ["TAB", "suit menu"],
      ["M", "medpack"],
      ["C", "stasis"],
      ["L", "locator"],
      ["SHIFT+L", "next destination"],
    ],
  },
];
