import { useEffect, useState } from "react";
import { shock, unlock } from "../../game/act";
import { cue } from "../../sound";
import { angleBetween, wrap } from "../suit/arcs";

/** Three tumblers, one try each at a time. */
export const TUMBLERS = 3;
/** Seconds to beat the lock. */
export const TIME = 20;
/** How close the pick must sit to a tumbler's angle, degrees. */
const TOLERANCE = 12;
/** A wrong try costs this many seconds. */
const PENALTY = 3;

function roll(): number[] {
  return Array.from({ length: TUMBLERS }, () =>
    Math.floor(Math.random() * 360),
  );
}

export type Bypass = {
  /** The pick, degrees (0 = up, clockwise). */
  angle: number;
  /** Tumblers locked so far. */
  solved: number;
  /** Seconds left. */
  time: number;
  /** 0..1: how near the pick sits to the next tumbler. */
  near: number;
  done: boolean;
  turn: (by: number) => void;
  point: (deg: number) => void;
  /** Try the current tumbler at the pick's angle. */
  attempt: () => void;
  /** Lock the next tumbler outright (the `hack solve` verb). */
  solve: () => void;
};

/** The bypass minigame: find each tumbler's hidden angle by feel (`near`),
 *  then try it. A miss shocks and costs time; running out of time shocks
 *  and resets the lock; the third tumbler unlocks the bulkhead. The clock
 *  runs only while `running`. Already `unlocked` = done. */
export function useBypass(running: boolean, unlocked: boolean): Bypass {
  const [angle, setAngle] = useState(0);
  const [targets, setTargets] = useState(roll);
  const [solved, setSolved] = useState(0);
  const [time, setTime] = useState(TIME);
  const done = unlocked || solved >= TUMBLERS;

  useEffect(() => {
    if (!running || done) return;
    const t = setInterval(() => setTime((s) => Math.max(0, s - 0.1)), 100);
    return () => clearInterval(t);
  }, [running, done]);

  // Out of time: the panel bites and the lock re-rolls.
  useEffect(() => {
    if (time > 0 || done) return;
    shock();
    setTargets(roll());
    setSolved(0);
    setTime(TIME);
  }, [time, done]);

  const lock = () => {
    const next = solved + 1;
    setSolved(next);
    cue("hackLock");
    if (next >= TUMBLERS) unlock();
  };

  const target = targets[Math.min(solved, TUMBLERS - 1)];
  return {
    angle,
    solved: done ? TUMBLERS : solved,
    time,
    near: done ? 0 : Math.max(0, 1 - angleBetween(angle, target) / 90),
    done,
    turn: (by) => setAngle((a) => wrap(a + by)),
    point: (deg) => setAngle(wrap(deg)),
    attempt: () => {
      if (done) return;
      if (angleBetween(angle, target) <= TOLERANCE) return lock();
      shock();
      setTime((s) => Math.max(0, s - PENALTY));
    },
    solve: () => {
      if (!done) lock();
    },
  };
}
