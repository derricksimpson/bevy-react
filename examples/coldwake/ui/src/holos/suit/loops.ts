import { useEffect } from "react";
import {
  useSharedValue,
  withRepeat,
  withTiming,
  type SharedValue,
} from "bevy-react";

/** A value Bevy swings between 1 and `low` every `ms` while `on` (a blink, a
 *  pulse), resting at 1 otherwise. React renders once; Bevy animates. */
export function useBlink(on: boolean, ms = 450, low = 0.2): SharedValue {
  const v = useSharedValue(1);
  useEffect(() => {
    v.value = on
      ? withRepeat(withTiming(low, { duration: ms, easing: "easeInOut" }), {
          reverse: true,
        })
      : withTiming(1, { duration: 150 });
  }, [on, v, ms, low]);
  return v;
}

/** An angle Bevy turns forever: a full turn every `secs` (negative turns
 *  the other way). */
export function useSpin(secs: number): SharedValue {
  const v = useSharedValue(0);
  useEffect(() => {
    v.value = withRepeat(
      withTiming(secs > 0 ? 360 : -360, {
        duration: Math.abs(secs) * 1000,
        easing: "linear",
      }),
    );
  }, [secs, v]);
  return v;
}
