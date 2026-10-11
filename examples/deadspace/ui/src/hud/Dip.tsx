import { useEffect } from "react";
import {
  useSharedValue,
  withDelay,
  withSequence,
  withTiming,
} from "bevy-react";

/** The cut between two fixtures: the screen fills with hologram static and
 *  clears again. Mounted for one dip (the parent keys it); the camera cuts
 *  under full cover, at `CUT_AT_SECS` in `tour.rs`. */
export function Dip() {
  const cover = useSharedValue(0);
  useEffect(() => {
    cover.value = withSequence(
      withTiming(1, { duration: 150, easing: "easeOut" }),
      withDelay(170, withTiming(0, { duration: 260, easing: "easeIn" })),
    );
  }, [cover]);
  return (
    <node
      style={{
        positionType: "absolute",
        left: 0,
        right: 0,
        top: 0,
        bottom: 0,
        opacity: { animated: cover },
        filter: { name: "holoStatic" },
      }}
    />
  );
}
