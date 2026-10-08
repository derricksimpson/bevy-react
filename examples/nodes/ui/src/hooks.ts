import { useEffect, useRef, useState } from "react";
import { bevy, on, type ReactEvents, type WindowSize } from "./bevy";

/** The UI viewport's logical size, streamed by the built-in `resize` event.
 *  `0×0` until the host answers. */
export function useWindowSize(): WindowSize {
  const [size, setSize] = useState<WindowSize>({ width: 0, height: 0 });
  useEffect(() => {
    bevy.window
      .size()
      .then(setSize)
      .catch(() => setSize({ width: 1440, height: 900 }));
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

/** The text field being typed into, if any: while one is open the editor's
 *  shortcuts are off (every key also arrives as a global `keyDown`), and a
 *  press elsewhere commits it. On `globalThis` to survive hot reloads. */
const typing: { commit: (() => void) | null } = ((
  globalThis as Record<string, unknown>
).__nodesTyping ??= { commit: null }) as never;

export const isTyping = () => typing.commit !== null;

/** Commit the open text field, if any (call on a press elsewhere). */
export function endTyping() {
  typing.commit?.();
}

export function startTyping(commit: () => void): () => void {
  typing.commit?.();
  typing.commit = commit;
  return () => {
    if (typing.commit === commit) typing.commit = null;
  };
}
