import { useSyncExternalStore } from "react";
import { reduce } from "./reduce";
import { initialGame, type Action, type Game } from "./state";

/** The one game store. Kept on `globalThis` so a hot reload (which re-runs
 *  this module) keeps the game. */
type Store = { game: Game; listeners: Set<() => void> };
const g = globalThis as { __deadspace?: Store };
const store: Store = (g.__deadspace ??= {
  game: initialGame(),
  listeners: new Set(),
});

/** The game now (outside React: event handlers, timers). */
export function getGame(): Game {
  return store.game;
}

/** Apply `action`; returns whether it changed anything (a refused action —
 *  no medpack, an empty clip — returns `false`). */
export function dispatch(action: Action): boolean {
  const next = reduce(store.game, action);
  if (next === store.game) return false;
  store.game = next;
  store.listeners.forEach((l) => l());
  return true;
}

function subscribe(listener: () => void) {
  store.listeners.add(listener);
  return () => store.listeners.delete(listener);
}

/** The game, or a slice of it (re-renders only when the slice changes by
 *  identity — select objects the reducer keeps when untouched). */
export function useGame(): Game;
export function useGame<T>(select: (g: Game) => T): T;
export function useGame<T>(select?: (g: Game) => T): T | Game {
  return useSyncExternalStore(subscribe, () =>
    select ? select(store.game) : store.game,
  );
}
