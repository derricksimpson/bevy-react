/** Everything the engineer can carry, buy or sell. The ids are shared with
 *  Rust: `preview/items.rs` builds a model per id (the turntable and the
 *  `icon-<id>` snapshots), and a Rust test checks the two lists match. */
export const ITEMS = {
  "medpack-s": {
    name: "Small Medpack",
    kind: "health",
    price: 1500,
    stack: 5,
    text: "Restores a quarter of the suit's vitals.",
  },
  "medpack-m": {
    name: "Medium Medpack",
    kind: "health",
    price: 3500,
    stack: 5,
    text: "Restores half of the suit's vitals.",
  },
  "medpack-l": {
    name: "Large Medpack",
    kind: "health",
    price: 7500,
    stack: 5,
    text: "Restores the suit's vitals in full.",
  },
  "stasis-pack": {
    name: "Stasis Pack",
    kind: "stasis",
    price: 2000,
    stack: 3,
    text: "Refills the stasis module.",
  },
  "air-canister": {
    name: "Air Canister",
    kind: "air",
    price: 1200,
    stack: 3,
    text: "Thirty seconds of air for the suit's tanks.",
  },
  "cutter-ammo": {
    name: "Arc Cutter Cells",
    kind: "ammo",
    price: 800,
    stack: 25,
    text: "Charge cells for the Arc Cutter. Ten shots a cell.",
  },
  "pulse-ammo": {
    name: "Pulse Driver Rounds",
    kind: "ammo",
    price: 1500,
    stack: 250,
    text: "Caseless rounds for the Pulse Driver.",
  },
  "rail-ammo": {
    name: "Rail Spikes",
    kind: "ammo",
    price: 2200,
    stack: 10,
    text: "Tungsten spikes for the Rail Spike.",
  },
  "power-cell": {
    name: "Power Cell",
    kind: "upgrade",
    price: 10000,
    stack: 10,
    text: "Powers one circuit node at an upgrade bench.",
  },
  semiconductor: {
    name: "Semiconductor",
    kind: "valuable",
    price: 3000,
    stack: 5,
    text: "Salvaged from a dead reactor. Worth something at a store.",
  },
  cutter: {
    name: "Arc Cutter",
    kind: "weapon",
    price: 0,
    stack: 1,
    text: "Halden mining tool. Three emitters cut a line of plasma.",
  },
  pulse: {
    name: "Pulse Driver",
    kind: "weapon",
    price: 12000,
    stack: 1,
    text: "Station security rifle. Fast, wide spread.",
  },
  rail: {
    name: "Rail Spike",
    kind: "weapon",
    price: 18000,
    stack: 1,
    text: "Survey spike launcher, repurposed. Pins what it hits.",
  },
} as const;

export type ItemId = keyof typeof ITEMS;
export type ItemKind = (typeof ITEMS)[ItemId]["kind"];
export const ITEM_IDS = Object.keys(ITEMS) as ItemId[];

export type WeaponId = "cutter" | "pulse" | "rail";
export const WEAPONS: WeaponId[] = ["cutter", "pulse", "rail"];

/** The ammo item each weapon loads, and how many rounds one of those items
 *  holds (clip sizes come from the bench, `stats` in `bench.ts`). */
export const WEAPON_AMMO: Record<WeaponId, { ammo: ItemId; perItem: number }> =
  {
    cutter: { ammo: "cutter-ammo", perItem: 10 },
    pulse: { ammo: "pulse-ammo", perItem: 25 },
    rail: { ammo: "rail-ammo", perItem: 4 },
  };

/** One inventory slot: an item and how many. */
export type Slot = { item: ItemId; count: number };

/** The suit's inventory: a fixed grid of slots (empty = `null`), the
 *  credits, and the station safe (no limit). */
export type Pack = {
  slots: (Slot | null)[];
  credits: number;
  safe: Slot[];
};

export const PACK_SLOTS = 12;

export function initialPack(): Pack {
  const slots: (Slot | null)[] = [
    { item: "medpack-s", count: 2 },
    { item: "medpack-m", count: 1 },
    { item: "stasis-pack", count: 1 },
    { item: "air-canister", count: 1 },
    { item: "cutter-ammo", count: 3 },
    { item: "power-cell", count: 2 },
    { item: "semiconductor", count: 2 },
  ];
  while (slots.length < PACK_SLOTS) slots.push(null);
  return {
    slots,
    credits: 14250,
    safe: [
      { item: "medpack-l", count: 1 },
      { item: "pulse-ammo", count: 1 },
    ],
  };
}

/** How many of `item` the pack holds. */
export function countItem(pack: Pack, item: ItemId): number {
  return pack.slots.reduce((n, s) => (s?.item === item ? n + s.count : n), 0);
}

/** The pack with `n` more of `item` (stacked, then into empty slots), or
 *  `null` if it doesn't fit. */
export function addItem(pack: Pack, item: ItemId, n = 1): Pack | null {
  const slots = pack.slots.map((s) => (s ? { ...s } : null));
  const stack = ITEMS[item].stack;
  let left = n;
  for (const s of slots) {
    if (left > 0 && s?.item === item && s.count < stack) {
      const take = Math.min(stack - s.count, left);
      s.count += take;
      left -= take;
    }
  }
  for (let i = 0; i < slots.length && left > 0; i++) {
    if (!slots[i]) {
      const take = Math.min(stack, left);
      slots[i] = { item, count: take };
      left -= take;
    }
  }
  return left > 0 ? null : { ...pack, slots };
}

/** The pack with `n` fewer of `item` (emptied slots go `null`), or `null`
 *  if it doesn't hold that many. */
export function takeItem(pack: Pack, item: ItemId, n = 1): Pack | null {
  if (countItem(pack, item) < n) return null;
  let left = n;
  const slots = pack.slots.map((s) => {
    if (!s || s.item !== item || left === 0) return s;
    const take = Math.min(s.count, left);
    left -= take;
    return s.count - take > 0 ? { ...s, count: s.count - take } : null;
  });
  return { ...pack, slots };
}

/** Credits a store pays for one `item`. */
export function sellPrice(item: ItemId): number {
  return Math.round(ITEMS[item].price * 0.5);
}

/** `14250` → `"14,250"`. */
export function credits(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}
