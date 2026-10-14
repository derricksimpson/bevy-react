import type { Kind as EngineKind, Signal } from "../bevy";

/** The modular synth's modules: every kind, its sockets and the knobs it
 *  starts with. Bevy plays them (`synth/` in Rust): the kinds are the
 *  engine's (`satisfies` below), the params its slots by name (a Rust test
 *  reads this file). */

export type Category = "timing" | "source" | "shaper" | "effect" | "output";

export const CATEGORIES: Record<Category, { label: string; color: string }> = {
  timing: { label: "Timing", color: "#9d7bff" },
  source: { label: "Sources", color: "#4cc2ff" },
  shaper: { label: "Shapers", color: "#ff7a59" },
  effect: { label: "Effects", color: "#57e38c" },
  output: { label: "Output", color: "#ffd166" },
};

export type Port = { name: string; signal: Signal };

export type KindDef = {
  title: string;
  category: Category;
  /** One line for the palette. */
  blurb: string;
  /** An SVG path in a 24×24 box, stroked. */
  icon: string;
  inputs: Port[];
  outputs: Port[];
  /** Card width and the body's height under the socket rows, world px. */
  width: number;
  body: number;
  params: Record<string, number>;
};

const port = (name: string, signal: Signal): Port => ({ name, signal });

export const KINDS = {
  clock: {
    title: "Clock",
    category: "timing",
    blurb: "Ticks on the beat, swung or straight",
    icon: "M21 12a9 9 0 1 1-18 0a9 9 0 1 1 18 0Z M12 7v5l3 2",
    inputs: [],
    outputs: [port("pulse", "gate")],
    width: 220,
    body: 78,
    params: { div: 1, swing: 0 },
  },
  sequencer: {
    title: "Sequencer",
    category: "timing",
    blurb: "Eight steps of melody",
    icon: "M4 19v-6 M9 19V8 M14 19v-4 M19 19V5",
    inputs: [port("clock", "gate")],
    outputs: [port("pitch", "cv"), port("gate", "gate")],
    width: 272,
    body: 96,
    params: {
      p0: 0.3,
      p1: 0.55,
      p2: 0.3,
      p3: 0.8,
      p4: 0.45,
      p5: 0.3,
      p6: 0.65,
      p7: 0.95,
      mask: 0b11011011,
    },
  },
  beat: {
    title: "Beat",
    category: "timing",
    blurb: "Four lanes of sixteenth hits",
    icon: "M3 6h18 M3 12h18 M3 18h18 M7 3.5v5 M15 9.5v5 M11 15.5v5",
    inputs: [port("clock", "gate")],
    outputs: [
      port("a", "gate"),
      port("b", "gate"),
      port("c", "gate"),
      port("d", "gate"),
    ],
    width: 340,
    body: 108,
    // A bar per lane, bit `i` the `i`-th sixteenth: four on the floor,
    // a backbeat, off-beat eighths.
    params: { k0: 0x1111, k1: 0x1010, k2: 0x4444, k3: 0 },
  },
  oscillator: {
    title: "Oscillator",
    category: "source",
    blurb: "A tone, four shapes, detunable",
    icon: "M1 12q3.5-8 7 0t7 0t7 0",
    inputs: [port("pitch", "cv"), port("fm", "cv")],
    outputs: [port("out", "audio")],
    width: 236,
    body: 150,
    params: { wave: 1, tune: 0.5, level: 0.8, detune: 0 },
  },
  noise: {
    title: "Noise",
    category: "source",
    blurb: "Hiss in three colors",
    icon: "M2 12l2-5 2 9 2-7 2 6 2-9 2 8 2-4 2 5 2-6 2 3",
    inputs: [],
    outputs: [port("out", "audio")],
    width: 212,
    body: 146,
    params: { color: 1, level: 0.5 },
  },
  lfo: {
    title: "LFO",
    category: "source",
    blurb: "A slow wobble, or random steps",
    icon: "M2 14q5-11 10 0t10 0",
    inputs: [],
    outputs: [port("out", "cv")],
    width: 212,
    body: 150,
    params: { shape: 0, rate: 0.55, depth: 0.6 },
  },
  drum: {
    title: "Drum",
    category: "source",
    blurb: "A kick, snare, clap or hat",
    icon: "M4 8.5a8 3 0 1 0 16 0a8 3 0 1 0-16 0Z M4 8.5v7c0 1.7 3.6 3 8 3s8-1.3 8-3v-7",
    inputs: [port("trig", "gate"), port("pitch", "cv")],
    outputs: [port("out", "audio")],
    width: 236,
    body: 140,
    params: { voice: 0, tune: 0.4, decay: 0.4, level: 0.8 },
  },
  envelope: {
    title: "Envelope",
    category: "shaper",
    blurb: "Attack, decay, sustain, release",
    icon: "M3 19L7 5l4 6h6l4 8",
    inputs: [port("gate", "gate")],
    outputs: [port("env", "cv")],
    width: 244,
    body: 132,
    params: { a: 0.12, d: 0.35, s: 0.55, r: 0.45 },
  },
  filter: {
    title: "Filter",
    category: "shaper",
    blurb: "Cuts the highs, the lows",
    icon: "M2 9h9c4 0 5 2 6 5s2 5 5 5",
    inputs: [port("in", "audio"), port("cutoff", "cv"), port("fm", "cv")],
    outputs: [port("out", "audio")],
    width: 236,
    body: 156,
    params: { mode: 0, cutoff: 0.55, res: 0.4, mod: 0.6 },
  },
  vca: {
    title: "VCA",
    category: "shaper",
    blurb: "Lets sound through as a voltage says",
    icon: "M5 4v16l14-8z M12 3v5.5",
    inputs: [port("in", "audio"), port("cv", "cv")],
    outputs: [port("out", "audio")],
    width: 212,
    body: 120,
    params: { gain: 0.8, curve: 0 },
  },
  glide: {
    title: "Glide",
    category: "shaper",
    blurb: "Slides from note to note",
    icon: "M3 17h5c4 0 4-10 8-10h5",
    inputs: [port("in", "cv")],
    outputs: [port("out", "cv")],
    width: 200,
    body: 112,
    params: { time: 0.3 },
  },
  mixer: {
    title: "Mixer",
    category: "effect",
    blurb: "Four into one",
    icon: "M6 4v16 M12 4v16 M18 4v16 M4 9h4 M10 15h4 M16 7h4",
    inputs: [
      port("ch 1", "audio"),
      port("ch 2", "audio"),
      port("ch 3", "audio"),
      port("ch 4", "audio"),
    ],
    outputs: [port("mix", "audio")],
    width: 220,
    body: 112,
    params: { l0: 0.8, l1: 0.6, l2: 0.6, l3: 0.6 },
  },
  delay: {
    title: "Delay",
    category: "effect",
    blurb: "Echoes, fading, in time",
    icon: "M4 6v12 M9 8v8 M14 10v4 M19 11v2",
    inputs: [port("in", "audio")],
    outputs: [port("out", "audio")],
    width: 236,
    body: 154,
    params: { time: 0.4, feedback: 0.45, mix: 0.3, sync: 1 },
  },
  reverb: {
    title: "Reverb",
    category: "effect",
    blurb: "A plate, in stereo",
    icon: "M10 12a2 2 0 1 0 4 0a2 2 0 1 0-4 0 M7.8 7.8a6 6 0 0 0 0 8.4 M16.2 7.8a6 6 0 0 1 0 8.4 M4.9 4.9a10 10 0 0 0 0 14.2 M19.1 4.9a10 10 0 0 1 0 14.2",
    inputs: [port("in", "audio")],
    outputs: [port("left", "audio"), port("right", "audio")],
    width: 236,
    body: 116,
    params: { size: 0.6, damp: 0.4, mix: 0.3 },
  },
  output: {
    title: "Output",
    category: "output",
    blurb: "Where it all ends up",
    icon: "M4 9h4l5-4v14l-5-4H4z M17 8a5 5 0 0 1 0 8 M20 5a9 9 0 0 1 0 14",
    inputs: [port("left", "audio"), port("right", "audio")],
    outputs: [],
    width: 224,
    body: 132,
    params: { volume: 0.7 },
  },
} satisfies Record<EngineKind, KindDef>;

export type Kind = keyof typeof KINDS;

export const kind = (k: Kind): KindDef => KINDS[k];

/** The card's fixed geometry (world px) — `NodeCard` renders exactly this,
 *  and the minimap, box select and framing measure with it. */
export const HEADER = 36;
export const ROW = 24;
export const ROWS_PAD = 6;
export const BODY_PAD = 12;

export function cardHeight(k: Kind): number {
  const d = kind(k);
  const rows = Math.max(d.inputs.length, d.outputs.length);
  return HEADER + (rows ? rows * ROW + ROWS_PAD * 2 : 0) + d.body + BODY_PAD;
}
