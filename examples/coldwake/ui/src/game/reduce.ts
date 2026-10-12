import { canInstall, stats } from "./bench";
import {
  addItem,
  countItem,
  ITEMS,
  sellPrice,
  takeItem,
  WEAPON_AMMO,
  type ItemId,
  type WeaponId,
} from "./items";
import { OBJECTIVES } from "./lore";
import type { Action, Game } from "./state";

/** How much each medpack restores. */
export const HEALS: Partial<Record<ItemId, number>> = {
  "medpack-s": 0.25,
  "medpack-m": 0.5,
  "medpack-l": 1,
};
/** One hit's damage before armor. */
const HIT = 0.15;
/** One stasis shot's cost. */
const STASIS_COST = 0.25;
/** Health lost per second with the tanks empty. */
const SUFFOCATE = 0.05;

/** A weapon's clip size with its bench upgrades. */
export function clipSize(g: Game, weapon: WeaponId): number {
  return stats(weapon, g.bench[weapon]).capacity;
}

/** Seconds of air a full tank holds (the RIG's air nodes). */
export function airMax(g: Game): number {
  return stats("rig", g.bench.rig).capacity;
}

/** Rounds left outside the clip for `weapon`. */
export function reserve(g: Game, weapon: WeaponId): number {
  const { ammo, perItem } = WEAPON_AMMO[weapon];
  return countItem(g.pack, ammo) * perItem;
}

/** The current objective (index into `OBJECTIVES`): the first step the
 *  game hasn't done yet, in whatever order he gets to them. */
export function objective(g: Game): number {
  if (!g.story.bought) return 0;
  if (g.bench.cutter.length === 0) return 1;
  if (g.fixtures.door === "locked") return 2;
  return OBJECTIVES.length - 1;
}

/** The game after `a`. A change that can't happen (no medpack, an empty
 *  clip, not enough credits) returns `g` itself: callers compare by
 *  identity to know an action was refused. */
export function reduce(g: Game, a: Action): Game {
  switch (a.type) {
    case "near": {
      const { near } = g.here;
      if (near.includes(a.surface) === a.on) return g;
      const next = a.on
        ? [...near, a.surface]
        : near.filter((s) => s !== a.surface);
      return { ...g, here: { ...g.here, near: next } };
    }
    case "stance": {
      const { aiming, mouse, screen, offer } = a;
      return { ...g, here: { ...g.here, aiming, mouse, screen, offer } };
    }
    case "menu":
      if (g.here.menu === a.open) return g;
      return { ...g, here: { ...g.here, menu: a.open } };
    case "introOver":
      return g.here.intro ? { ...g, here: { ...g.here, intro: false } } : g;

    case "hit": {
      const armor = stats("rig", g.bench.rig).damage / 100;
      const health = Math.max(0, g.suit.health - HIT * (1 - armor));
      return { ...g, suit: { ...g.suit, health } };
    }
    case "heal": {
      const amount = HEALS[a.item];
      if (!amount || g.suit.health >= 1) return g;
      const pack = takeItem(g.pack, a.item);
      if (!pack) return g;
      const health = Math.min(1, g.suit.health + amount);
      return { ...g, pack, suit: { ...g.suit, health } };
    }
    case "stasis":
      if (g.suit.stasis < STASIS_COST) return g;
      return { ...g, suit: { ...g.suit, stasis: g.suit.stasis - STASIS_COST } };
    case "stasisRefill": {
      if (g.suit.stasis >= 1) return g;
      const pack = takeItem(g.pack, "stasis-pack");
      return pack ? { ...g, pack, suit: { ...g.suit, stasis: 1 } } : g;
    }
    case "vacuum":
      if (g.suit.vacuum === a.on) return g;
      return {
        ...g,
        // The tanks refill in air, so a vent always starts them full.
        suit: { ...g.suit, vacuum: a.on, air: airMax(g) },
      };
    case "air": {
      if (!g.suit.vacuum) return g;
      const air = Math.max(0, g.suit.air - a.dt);
      const health =
        air > 0 ? g.suit.health : Math.max(0, g.suit.health - SUFFOCATE * a.dt);
      return { ...g, suit: { ...g.suit, air, health } };
    }
    case "airRefill": {
      const max = airMax(g);
      if (g.suit.air >= max) return g;
      const pack = takeItem(g.pack, "air-canister");
      if (!pack) return g;
      return {
        ...g,
        pack,
        suit: { ...g.suit, air: Math.min(max, g.suit.air + 30) },
      };
    }

    case "fire": {
      const { weapon, clip } = g.arms;
      if (clip[weapon] <= 0) return g;
      return {
        ...g,
        arms: { ...g.arms, clip: { ...clip, [weapon]: clip[weapon] - 1 } },
      };
    }
    case "reload": {
      const { weapon, clip } = g.arms;
      const size = clipSize(g, weapon);
      if (clip[weapon] >= size) return g;
      const pack = takeItem(g.pack, WEAPON_AMMO[weapon].ammo);
      if (!pack) return g;
      return {
        ...g,
        pack,
        arms: { ...g.arms, clip: { ...clip, [weapon]: size } },
      };
    }
    case "equip":
      if (!g.arms.owned.includes(a.weapon) || g.arms.weapon === a.weapon)
        return g;
      return { ...g, arms: { ...g.arms, weapon: a.weapon } };

    case "buy": {
      const item = ITEMS[a.item];
      if (item.price > g.pack.credits) return g;
      const credits = g.pack.credits - item.price;
      const story = { ...g.story, bought: true };
      if (item.kind === "weapon") {
        const weapon = a.item as WeaponId;
        if (g.arms.owned.includes(weapon)) return g;
        return {
          ...g,
          story,
          pack: { ...g.pack, credits },
          arms: { ...g.arms, owned: [...g.arms.owned, weapon] },
        };
      }
      const pack = addItem(g.pack, a.item);
      return pack ? { ...g, story, pack: { ...pack, credits } } : g;
    }
    case "sell": {
      const slot = g.pack.slots[a.slot];
      if (!slot) return g;
      const pack = takeItem(g.pack, slot.item);
      if (!pack) return g;
      return {
        ...g,
        pack: { ...pack, credits: pack.credits + sellPrice(slot.item) },
      };
    }
    case "stow": {
      const slot = g.pack.slots[a.slot];
      if (!slot) return g;
      const slots = g.pack.slots.map((s, i) => (i === a.slot ? null : s));
      const kept = g.pack.safe.find((s) => s.item === slot.item);
      const safe = kept
        ? g.pack.safe.map((s) =>
            s === kept ? { ...s, count: s.count + slot.count } : s,
          )
        : [...g.pack.safe, slot];
      return { ...g, pack: { ...g.pack, slots, safe } };
    }
    case "retrieve": {
      const stored = g.pack.safe[a.index];
      if (!stored) return g;
      const pack = addItem(g.pack, stored.item, stored.count);
      if (!pack) return g;
      return {
        ...g,
        pack: { ...pack, safe: g.pack.safe.filter((_, i) => i !== a.index) },
      };
    }

    case "install": {
      const installed = g.bench[a.target];
      if (!canInstall(a.target, installed, a.node)) return g;
      const pack = takeItem(g.pack, "power-cell");
      if (!pack) return g;
      return {
        ...g,
        pack,
        bench: { ...g.bench, [a.target]: [...installed, a.node] },
      };
    }

    case "read":
      if (g.story.read.includes(a.log)) return g;
      return { ...g, story: { ...g.story, read: [...g.story.read, a.log] } };

    case "unlock":
      if (g.fixtures.door !== "locked") return g;
      return { ...g, fixtures: { ...g.fixtures, door: "unlocked" } };
    case "opened":
      return { ...g, fixtures: { ...g.fixtures, door: "open" } };
    case "save": {
      const id = (g.fixtures.saves.at(-1)?.id ?? 0) + 1;
      const copy: Game = { ...g, fixtures: { ...g.fixtures, saves: [] } };
      const slot = {
        id,
        time: a.time,
        objective: objective(g),
        game: copy,
      };
      return {
        ...g,
        fixtures: { ...g.fixtures, saves: [...g.fixtures.saves, slot] },
      };
    }
    case "load": {
      const slot = g.fixtures.saves.find((s) => s.id === a.id);
      if (!slot) return g;
      // The room itself doesn't rewind: where he stands, the doors and
      // the elevator stay as they are.
      return {
        ...slot.game,
        here: g.here,
        fixtures: { ...g.fixtures, locator: slot.game.fixtures.locator },
      };
    }
    case "call": {
      const { elevator } = g.fixtures;
      if (elevator.target !== null || elevator.floor === a.floor) return g;
      return {
        ...g,
        fixtures: { ...g.fixtures, elevator: { ...elevator, target: a.floor } },
      };
    }
    case "arrived":
      return {
        ...g,
        fixtures: { ...g.fixtures, elevator: { floor: a.floor, target: null } },
      };
    case "locator":
      return { ...g, fixtures: { ...g.fixtures, locator: a.to } };
  }
}
