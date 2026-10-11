import { useEffect, useRef } from "react";
import { on, type KeyboardEventData, type ReactEvents } from "./bevy";

/** Listen to a Bevy event while mounted; `run` always sees the latest
 *  render's state. */
export function useEvent<K extends keyof ReactEvents>(
  name: K,
  run: (value: ReactEvents[K]) => void,
) {
  const latest = useRef(run);
  latest.current = run;
  useEffect(() => on(name, (value) => latest.current(value)), [name]);
}

/** Key presses (no auto-repeat unless `repeat`) while mounted and `active`.
 *  `run` gets the event; match on `e.code` (layout-independent) or
 *  `e.key`. */
export function useKeys(
  active: boolean,
  run: (e: KeyboardEventData) => void,
  repeat = false,
) {
  useEvent("keyDown", (e) => {
    if (!active || (e.repeat && !repeat)) return;
    run(e);
  });
}

/** A scripted step from `--shoot … --do "<secs> <verb> [arg]"` (the
 *  `debug.act` event): runs `run(arg)` for this component's `verb`. */
export function useDebug(verb: string, run: (arg: string) => void) {
  useEvent("debug.act", ({ action }) => {
    const [head, ...rest] = action.split(" ");
    if (head === verb) run(rest.join(" "));
  });
}
