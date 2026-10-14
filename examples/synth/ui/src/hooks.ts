import { useEffect, useRef, useState } from "react";
import { bevy, on, type ReactEvents, type WindowSize } from "./bevy";

/** The window's logical size, streamed by the built-in `resize` event.
 *  `0×0` until the host answers. */
export function useWindowSize(): WindowSize {
  const [size, setSize] = useState<WindowSize>({ width: 0, height: 0 });
  useEffect(() => {
    bevy.window
      .size()
      .then(setSize)
      .catch(() => {});
    return bevy.on("resize", setSize);
  }, []);
  return size;
}

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

/** A scripted step from `--shoot … --do "<secs> <verb> [args]"` (the
 *  `debug.act` event): runs `run(args)` for this component's `verb`. */
export function useDebug(verb: string, run: (args: string[]) => void) {
  useEvent("debug.act", ({ action }) => {
    const [head, ...rest] = action.split(" ");
    if (head === verb) run(rest);
  });
}

/** A ref that always holds the latest value: handlers that outlive a render
 *  (gesture callbacks, timers) read current state through it. */
export function useLatest<T>(value: T) {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}
