import { type Kind, KINDS } from "./catalog";
import { EMPTY, type Graph, type GraphNode, type Link, arrange } from "./model";

/** The patches the top bar loads, each with its tempo. They are laid out
 *  as Arrange would (so L moves nothing); nodes pop in left to right, and
 *  the wires are plugged in a moment later (`WIRE_DELAY`), so they grow out
 *  of nodes that are already there. */

type Spec = [id: string, kind: Kind, params?: Record<string, number>];

export type Preset = {
  name: string;
  /** One line for the menu. */
  blurb: string;
  bpm: number;
  graph: Graph;
  links: Link[];
};

/** A sequencer step's height for scale degree `n` (0 the root, 5 an octave
 *  up, 10 two): the sequencer snaps heights to minor pentatonic. */
const deg = (...ns: number[]) =>
  Object.fromEntries(ns.map((n, i) => [`p${i}`, n / 10]));

/** An oscillator's `tune` for `semis` from A2 (-12..12). */
const tune = (semis: number) => (semis + 12) / 24;

/** A beat lane from a bar drawn as `x` and `.`, sixteenths left to right. */
const lane = (bar: string) =>
  [...bar.replace(/ /g, "")].reduce(
    (m, c, i) => (c === "x" ? m | (1 << i) : m),
    0,
  );

/** `"a.0>b.1"`: node `a`'s output 0 into node `b`'s input 1. */
function preset(
  name: string,
  blurb: string,
  bpm: number,
  specs: Spec[],
  wires: string[],
): Preset {
  const nodes: Record<string, GraphNode> = {};
  specs.forEach(([id, kind, params], i) => {
    nodes[id] = {
      id,
      kind,
      x: 0,
      // Arrange keeps the sources in this order.
      y: i,
      params: { ...KINDS[kind].params, ...params },
      z: i + 1,
      enter: 0,
    };
  });
  const links = wires.map((w, i) => {
    const [from, to] = w.split(">").map((end) => {
      const [node, port] = end.split(".");
      return { node, port: Number(port) };
    });
    return { id: `${name}-w${i}`, from, to };
  });
  const at = arrange({ ...EMPTY, nodes, links });
  const xs = Object.values(at).map((p) => p.x);
  const [left, right] = [Math.min(...xs), Math.max(...xs)];
  Object.values(nodes).forEach((n, i) => {
    Object.assign(n, at[n.id]);
    n.enter =
      60 + ((n.x - left) / Math.max(1, right - left)) * 420 + (i % 2) * 40;
  });
  const graph = {
    nodes,
    links: [],
    selected: [],
    seq: 1,
    top: specs.length + 1,
  };
  return { name, blurb, bpm, graph, links };
}

/** A fresh copy of `p` for its `tag`-th load: ids never repeat, so cards
 *  still shrinking away from the last patch never meet their successors. */
export function instantiate(
  p: Preset,
  tag: number,
): { graph: Graph; links: Link[] } {
  const id = (n: string) => `${n}~${tag}`;
  const nodes: Record<string, GraphNode> = {};
  for (const n of Object.values(p.graph.nodes))
    nodes[id(n.id)] = { ...n, id: id(n.id) };
  const links = p.links.map((l) => ({
    id: id(l.id),
    from: { ...l.from, node: id(l.from.node) },
    to: { ...l.to, node: id(l.to.node) },
  }));
  return { graph: { ...p.graph, nodes }, links };
}

/** When the wires go in after a preset loads, ms. */
export const WIRE_DELAY = 520;

export const PRESETS: Preset[] = [
  preset(
    "Night Drive",
    "A detuned saw arpeggio, dotted echoes, a kick",
    112,
    [
      ["clk", "clock", { div: 1 }],
      ["lfo", "lfo", { shape: 0, rate: 0.2, depth: 0.3 }],
      ["seq", "sequencer", { ...deg(0, 3, 5, 6, 5, 3, 7, 6), mask: 0xff }],
      [
        "beat",
        "beat",
        {
          k0: lane("x... x... x... x..."),
          k1: 0,
          k2: lane("..x. ..x. ..x. ..x."),
          k3: 0,
        },
      ],
      [
        "osc",
        "oscillator",
        { wave: 1, tune: tune(0), level: 0.8, detune: 0.5 },
      ],
      ["env", "envelope", { a: 0, d: 0.3, s: 0.12, r: 0.25 }],
      ["kick", "drum", { voice: 0, tune: 0.3, decay: 0.3, level: 0.9 }],
      ["hat", "drum", { voice: 3, tune: 0.55, decay: 0.15, level: 0.6 }],
      ["flt", "filter", { mode: 0, cutoff: 0.36, res: 0.45, mod: 0.55 }],
      ["vca", "vca", { gain: 1, curve: 0 }],
      ["dly", "delay", { time: 0.4, feedback: 0.4, mix: 0.28, sync: 1 }],
      ["mix", "mixer", { l0: 0.85, l1: 0.75, l2: 0.5, l3: 0 }],
      ["rev", "reverb", { size: 0.55, damp: 0.45, mix: 0.25 }],
      ["out", "output", { volume: 0.8 }],
    ],
    [
      "clk.0>seq.0",
      "clk.0>beat.0",
      "seq.0>osc.0",
      "seq.1>env.0",
      "osc.0>flt.0",
      "env.0>flt.1",
      "lfo.0>flt.2",
      "flt.0>vca.0",
      "env.0>vca.1",
      "vca.0>dly.0",
      "beat.0>kick.0",
      "beat.2>hat.0",
      "dly.0>mix.0",
      "kick.0>mix.1",
      "hat.0>mix.2",
      "mix.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Acid Line",
    "A squelching, sliding bassline over a kick and claps",
    126,
    [
      ["clk", "clock", { div: 2, swing: 0.15 }],
      ["lfo", "lfo", { shape: 0, rate: 0.22, depth: 0.35 }],
      ["seq", "sequencer", { ...deg(0, 5, 0, 3, 0, 6, 5, 4), mask: 0xff }],
      [
        "beat",
        "beat",
        {
          k0: lane("x... x... x... x..."),
          k1: lane(".... x... .... x..."),
          k2: lane(".xxx .xxx .xxx .xxx"),
          k3: lane("x.xx x.x. xx.x x.xx"),
        },
      ],
      ["glide", "glide", { time: 0.22 }],
      ["kick", "drum", { voice: 0, tune: 0.35, decay: 0.35, level: 0.9 }],
      ["clap", "drum", { voice: 2, tune: 0.4, decay: 0.3, level: 0.7 }],
      ["hat", "drum", { voice: 3, tune: 0.6, decay: 0.08, level: 0.4 }],
      ["env", "envelope", { a: 0, d: 0.32, s: 0, r: 0.12 }],
      [
        "osc",
        "oscillator",
        { wave: 1, tune: tune(-12), level: 0.9, detune: 0 },
      ],
      ["flt", "filter", { mode: 0, cutoff: 0.26, res: 0.82, mod: 0.62 }],
      ["vca", "vca", { gain: 1, curve: 0 }],
      ["mix", "mixer", { l0: 0.8, l1: 0.8, l2: 0.5, l3: 0.3 }],
      ["rev", "reverb", { size: 0.35, damp: 0.5, mix: 0.15 }],
      ["out", "output", { volume: 0.7 }],
    ],
    [
      "clk.0>seq.0",
      "clk.0>beat.0",
      "seq.0>glide.0",
      "glide.0>osc.0",
      "beat.0>kick.0",
      "beat.1>clap.0",
      "beat.2>hat.0",
      "beat.3>env.0",
      "osc.0>flt.0",
      "env.0>flt.1",
      "lfo.0>flt.2",
      "flt.0>vca.0",
      "env.0>vca.1",
      "vca.0>mix.0",
      "kick.0>mix.1",
      "clap.0>mix.2",
      "hat.0>mix.3",
      "mix.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Boom Bap",
    "A swung beat and a round bass, a small room",
    88,
    [
      ["clk", "clock", { div: 2, swing: 0.55 }],
      ["seq", "sequencer", { ...deg(0, 0, 3, 3, 2, 2, 4, 3), mask: 0xff }],
      [
        "beat",
        "beat",
        {
          k0: lane("x... ...x ..x. ...."),
          k1: lane(".... x... .... x..."),
          k2: lane("x.x. x.x. x.x. x.x."),
          k3: lane("x..x ...x ..x. ..x."),
        },
      ],
      [
        "osc",
        "oscillator",
        { wave: 1, tune: tune(-12), level: 1, detune: 0.1 },
      ],
      ["env", "envelope", { a: 0.05, d: 0.4, s: 0.3, r: 0.2 }],
      ["kick", "drum", { voice: 0, tune: 0.28, decay: 0.45, level: 0.95 }],
      ["snare", "drum", { voice: 1, tune: 0.45, decay: 0.35, level: 0.75 }],
      ["hat", "drum", { voice: 3, tune: 0.45, decay: 0.1, level: 0.45 }],
      ["flt", "filter", { mode: 0, cutoff: 0.3, res: 0.25, mod: 0.35 }],
      ["vca", "vca", { gain: 1, curve: 0 }],
      ["mix", "mixer", { l0: 0.85, l1: 0.65, l2: 0.35, l3: 0.7 }],
      ["rev", "reverb", { size: 0.3, damp: 0.6, mix: 0.18 }],
      ["out", "output", { volume: 0.8 }],
    ],
    [
      "clk.0>seq.0",
      "clk.0>beat.0",
      "seq.0>osc.0",
      "beat.3>env.0",
      "beat.0>kick.0",
      "beat.1>snare.0",
      "beat.2>hat.0",
      "osc.0>flt.0",
      "env.0>flt.1",
      "flt.0>vca.0",
      "env.0>vca.1",
      "kick.0>mix.0",
      "snare.0>mix.1",
      "hat.0>mix.2",
      "vca.0>mix.3",
      "mix.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Dub Chords",
    "Minor sevenths in a long echo, a kick, a little hiss",
    118,
    [
      ["clk", "clock", { div: 1, swing: 0.2 }],
      ["seq", "sequencer", { ...deg(5, 5, 5, 5, 7, 7, 7, 7), mask: 0xff }],
      ["lfo", "lfo", { shape: 0, rate: 0.2, depth: 0.45 }],
      [
        "beat",
        "beat",
        {
          k0: lane("x... x... x... x..."),
          k1: 0,
          k2: lane("..x. ..x. ..x. ..x."),
          k3: lane("..x. .... ..x. ...."),
        },
      ],
      [
        "root",
        "oscillator",
        { wave: 1, tune: tune(-12), level: 1, detune: 0.15 },
      ],
      [
        "sev",
        "oscillator",
        { wave: 1, tune: tune(-2), level: 0.85, detune: 0.15 },
      ],
      [
        "third",
        "oscillator",
        { wave: 1, tune: tune(3), level: 0.85, detune: 0.15 },
      ],
      ["env", "envelope", { a: 0.02, d: 0.4, s: 0, r: 0.3 }],
      ["kick", "drum", { voice: 0, tune: 0.25, decay: 0.45, level: 0.75 }],
      ["hat", "drum", { voice: 3, tune: 0.4, decay: 0.35, level: 0.4 }],
      ["hiss", "noise", { color: 1, level: 0.3 }],
      ["keys", "mixer", { l0: 1, l1: 0.9, l2: 0.9, l3: 0 }],
      ["flt", "filter", { mode: 0, cutoff: 0.45, res: 0.35, mod: 0.35 }],
      ["vca", "vca", { gain: 1, curve: 0 }],
      ["dly", "delay", { time: 0.4, feedback: 0.62, mix: 0.45, sync: 1 }],
      ["mix", "mixer", { l0: 1, l1: 0.7, l2: 0.5, l3: 0.05 }],
      ["rev", "reverb", { size: 0.85, damp: 0.55, mix: 0.35 }],
      ["out", "output", { volume: 0.8 }],
    ],
    [
      "clk.0>seq.0",
      "clk.0>beat.0",
      "seq.0>root.0",
      "seq.0>sev.0",
      "seq.0>third.0",
      "root.0>keys.0",
      "sev.0>keys.1",
      "third.0>keys.2",
      "keys.0>flt.0",
      "beat.3>env.0",
      "env.0>flt.1",
      "lfo.0>flt.2",
      "flt.0>vca.0",
      "env.0>vca.1",
      "vca.0>dly.0",
      "beat.0>kick.0",
      "beat.2>hat.0",
      "dly.0>mix.0",
      "kick.0>mix.1",
      "hat.0>mix.2",
      "hiss.0>mix.3",
      "mix.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Jungle",
    "A chopped break at 172 and a rolling reese bass",
    172,
    [
      ["clk", "clock", { div: 2 }],
      ["lfo", "lfo", { shape: 0, rate: 0.35, depth: 0.5 }],
      ["seq", "sequencer", { ...deg(0, 0, 0, 0, 0, 0, 4, 3), mask: 0xff }],
      [
        "beat",
        "beat",
        {
          k0: lane("x.x. .... ..x. ...."),
          k1: lane(".... x... .... x..."),
          k2: lane(".... ...x .x.. ...x"),
          k3: lane("x... .... ..x. ...."),
        },
      ],
      [
        "osc",
        "oscillator",
        { wave: 1, tune: tune(-12), level: 0.9, detune: 0.9 },
      ],
      ["env", "envelope", { a: 0, d: 0.5, s: 0.7, r: 0.55 }],
      ["kick", "drum", { voice: 0, tune: 0.35, decay: 0.3, level: 0.9 }],
      ["snare", "drum", { voice: 1, tune: 0.55, decay: 0.35, level: 0.8 }],
      ["ghost", "drum", { voice: 1, tune: 0.6, decay: 0.15, level: 0.35 }],
      ["hat", "drum", { voice: 3, tune: 0.7, decay: 0.12, level: 0.4 }],
      ["flt", "filter", { mode: 0, cutoff: 0.33, res: 0.3, mod: 0.3 }],
      ["vca", "vca", { gain: 1, curve: 0 }],
      ["drums", "mixer", { l0: 0.8, l1: 0.8, l2: 0.6, l3: 0.5 }],
      ["mix", "mixer", { l0: 1, l1: 0.72, l2: 0, l3: 0 }],
      ["rev", "reverb", { size: 0.4, damp: 0.5, mix: 0.15 }],
      ["out", "output", { volume: 0.88 }],
    ],
    [
      "clk.0>seq.0",
      "clk.0>beat.0",
      "seq.0>osc.0",
      "beat.3>env.0",
      "osc.0>flt.0",
      "env.0>flt.1",
      "lfo.0>flt.2",
      "flt.0>vca.0",
      "env.0>vca.1",
      "beat.0>kick.0",
      "beat.1>snare.0",
      "beat.2>ghost.0",
      "seq.1>hat.0",
      "kick.0>drums.0",
      "snare.0>drums.1",
      "ghost.0>drums.2",
      "hat.0>drums.3",
      "drums.0>mix.0",
      "vca.0>mix.1",
      "mix.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Half Time",
    "Sliding 808s, wandering hats, a clap on three, a dark bell",
    140,
    [
      ["clk", "clock", { div: 2 }],
      ["seq", "sequencer", { ...deg(0, 0, 0, 0, 0, 0, 2, 3), mask: 0xff }],
      ["melody", "sequencer", { ...deg(10, 9, 9, 8, 7, 8, 8, 6), mask: 0xff }],
      [
        "beat",
        "beat",
        {
          k0: lane("x... ...x ..x. ...."),
          k1: lane(".... .... x... ...."),
          k2: lane("x.x. x.xx x.x. xxxx"),
          k3: lane("x..x ..x. x..x ..x."),
        },
      ],
      ["wander", "lfo", { shape: 3, rate: 0.75, depth: 0.12 }],
      ["glide", "glide", { time: 0.35 }],
      ["sub", "oscillator", { wave: 2, tune: tune(-12), level: 1, detune: 0 }],
      ["senv", "envelope", { a: 0, d: 0.8, s: 0, r: 0.55 }],
      [
        "bell",
        "oscillator",
        { wave: 3, tune: tune(12), level: 0.8, detune: 0 },
      ],
      ["benv", "envelope", { a: 0, d: 0.4, s: 0, r: 0.4 }],
      ["clap", "drum", { voice: 2, tune: 0.45, decay: 0.45, level: 0.8 }],
      ["hat", "drum", { voice: 3, tune: 0.65, decay: 0.06, level: 0.6 }],
      ["flt", "filter", { mode: 0, cutoff: 0.28, res: 0.2, mod: 0.4 }],
      ["svca", "vca", { gain: 1, curve: 0 }],
      ["bvca", "vca", { gain: 1, curve: 1 }],
      ["dly", "delay", { time: 0.8, feedback: 0.4, mix: 0.3, sync: 1 }],
      ["mix", "mixer", { l0: 0.9, l1: 0.75, l2: 0.55, l3: 0.85 }],
      ["rev", "reverb", { size: 0.6, damp: 0.5, mix: 0.25 }],
      ["out", "output", { volume: 0.75 }],
    ],
    [
      "clk.0>seq.0",
      "clk.0>melody.0",
      "clk.0>beat.0",
      "seq.0>glide.0",
      "glide.0>sub.0",
      "beat.0>senv.0",
      "sub.0>flt.0",
      "senv.0>flt.1",
      "flt.0>svca.0",
      "senv.0>svca.1",
      "melody.0>bell.0",
      "beat.3>benv.0",
      "bell.0>bvca.0",
      "benv.0>bvca.1",
      "bvca.0>dly.0",
      "beat.1>clap.0",
      "beat.2>hat.0",
      "wander.0>hat.1",
      "svca.0>mix.0",
      "clap.0>mix.1",
      "hat.0>mix.2",
      "dly.0>mix.3",
      "mix.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Arcade",
    "An 8-bit loop: a square lead, a triangle bass",
    150,
    [
      ["clk", "clock", { div: 1 }],
      ["vib", "lfo", { shape: 0, rate: 0.78, depth: 0.12 }],
      ["lead", "sequencer", { ...deg(5, 6, 8, 10, 9, 8, 7, 8), mask: 0xff }],
      ["bass", "sequencer", { ...deg(0, 5, 0, 5, 3, 8, 4, 9), mask: 0xff }],
      [
        "beat",
        "beat",
        {
          k0: lane("x... ..x. x... ...."),
          k1: lane(".... x... .... x..."),
          k2: lane("x.x. x.x. x.x. x.x."),
          k3: 0,
        },
      ],
      ["sq", "oscillator", { wave: 2, tune: tune(12), level: 0.9, detune: 0 }],
      ["env", "envelope", { a: 0, d: 0.35, s: 0.6, r: 0.1 }],
      ["tri", "oscillator", { wave: 3, tune: tune(-12), level: 1, detune: 0 }],
      ["kick", "drum", { voice: 0, tune: 0.55, decay: 0.2, level: 0.8 }],
      ["snare", "drum", { voice: 1, tune: 0.7, decay: 0.3, level: 0.9 }],
      ["hat", "drum", { voice: 3, tune: 0.75, decay: 0.08, level: 0.6 }],
      ["vca", "vca", { gain: 1, curve: 0 }],
      ["dly", "delay", { time: 0.2, feedback: 0.25, mix: 0.22, sync: 1 }],
      ["drums", "mixer", { l0: 0.8, l1: 1, l2: 0.7, l3: 0 }],
      ["mix", "mixer", { l0: 0.75, l1: 0.45, l2: 0.9, l3: 0 }],
      ["out", "output", { volume: 0.8 }],
    ],
    [
      "clk.0>lead.0",
      "clk.0>bass.0",
      "clk.0>beat.0",
      "lead.0>sq.0",
      "vib.0>sq.1",
      "lead.1>env.0",
      "sq.0>vca.0",
      "env.0>vca.1",
      "vca.0>dly.0",
      "bass.0>tri.0",
      "beat.0>kick.0",
      "beat.1>snare.0",
      "beat.2>hat.0",
      "kick.0>drums.0",
      "snare.0>drums.1",
      "hat.0>drums.2",
      "dly.0>mix.0",
      "tri.0>mix.1",
      "drums.0>mix.2",
      "mix.0>out.0",
    ],
  ),
  preset(
    "Electro",
    "A tuned 808 kick playing the bassline, claps, robot stabs",
    126,
    [
      ["clk", "clock", { div: 2, swing: 0.1 }],
      ["bass", "sequencer", { ...deg(0, 0, 0, 5, 0, 3, 0, 4), mask: 0xff }],
      ["riff", "sequencer", { ...deg(5, 8, 5, 10, 9, 5, 7, 5), mask: 0xff }],
      [
        "beat",
        "beat",
        {
          k0: lane("x... ..x. ..x. .x.."),
          k1: lane(".... x... .... x..."),
          k2: lane("x.x. x.xx x.x. x.xx"),
          k3: lane("..x. ..x. .x.. x..."),
        },
      ],
      // TUNE 0.3675 is 55 Hz: the bassline's A.
      ["kick", "drum", { voice: 0, tune: 0.3675, decay: 0.7, level: 0.95 }],
      ["clap", "drum", { voice: 2, tune: 0.5, decay: 0.4, level: 0.75 }],
      ["hat", "drum", { voice: 3, tune: 0.6, decay: 0.07, level: 0.45 }],
      ["osc", "oscillator", { wave: 1, tune: tune(0), level: 1, detune: 0.3 }],
      ["env", "envelope", { a: 0, d: 0.3, s: 0, r: 0.15 }],
      ["flt", "filter", { mode: 0, cutoff: 0.48, res: 0.6, mod: 0.5 }],
      ["vca", "vca", { gain: 1, curve: 0 }],
      ["dly", "delay", { time: 0.4, feedback: 0.35, mix: 0.25, sync: 1 }],
      ["mix", "mixer", { l0: 0.5, l1: 0.75, l2: 0.55, l3: 1 }],
      ["rev", "reverb", { size: 0.35, damp: 0.5, mix: 0.15 }],
      ["out", "output", { volume: 0.75 }],
    ],
    [
      "clk.0>bass.0",
      "clk.0>riff.0",
      "clk.0>beat.0",
      "beat.0>kick.0",
      "bass.0>kick.1",
      "beat.1>clap.0",
      "beat.2>hat.0",
      "riff.0>osc.0",
      "beat.3>env.0",
      "osc.0>flt.0",
      "env.0>flt.1",
      "flt.0>vca.0",
      "env.0>vca.1",
      "vca.0>dly.0",
      "kick.0>mix.0",
      "clap.0>mix.1",
      "hat.0>mix.2",
      "dly.0>mix.3",
      "mix.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Music Box",
    "Plucked triangles, echoes and a large plate",
    96,
    [
      ["clk", "clock", { div: 1 }],
      [
        "seq",
        "sequencer",
        { ...deg(5, 7, 9, 10, 9, 7, 8, 6), mask: 0b10110111 },
      ],
      ["lfo", "lfo", { shape: 0, rate: 0.8, depth: 0.06 }],
      ["osc", "oscillator", { wave: 3, tune: tune(3), level: 1, detune: 0 }],
      ["env", "envelope", { a: 0, d: 0.38, s: 0, r: 0.35 }],
      ["vca", "vca", { gain: 1, curve: 1 }],
      ["dly", "delay", { time: 0.8, feedback: 0.45, mix: 0.32, sync: 1 }],
      ["rev", "reverb", { size: 0.82, damp: 0.25, mix: 0.45 }],
      ["out", "output", { volume: 1 }],
    ],
    [
      "clk.0>seq.0",
      "seq.0>osc.0",
      "lfo.0>osc.1",
      "seq.1>env.0",
      "osc.0>vca.0",
      "env.0>vca.1",
      "vca.0>dly.0",
      "dly.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset(
    "Tide",
    "A drone in fifths, swelling under a slow sweep",
    64,
    [
      ["lfo", "lfo", { shape: 0, rate: 0.1, depth: 0.75 }],
      ["drift", "lfo", { shape: 1, rate: 0.3, depth: 0.12 }],
      [
        "osa",
        "oscillator",
        { wave: 1, tune: tune(-7), level: 0.8, detune: 0.5 },
      ],
      [
        "osb",
        "oscillator",
        { wave: 1, tune: tune(0), level: 0.6, detune: 0.4 },
      ],
      ["nse", "noise", { color: 2, level: 0.35 }],
      ["mix", "mixer", { l0: 0.7, l1: 0.55, l2: 0.35, l3: 0 }],
      ["flt", "filter", { mode: 0, cutoff: 0.4, res: 0.42, mod: 0 }],
      ["dly", "delay", { time: 1, feedback: 0.5, mix: 0.3, sync: 1 }],
      ["rev", "reverb", { size: 0.95, damp: 0.3, mix: 0.6 }],
      ["out", "output", { volume: 0.7 }],
    ],
    [
      "drift.0>osb.1",
      "osa.0>mix.0",
      "osb.0>mix.1",
      "nse.0>mix.2",
      "mix.0>flt.0",
      "lfo.0>flt.2",
      "flt.0>dly.0",
      "dly.0>rev.0",
      "rev.0>out.0",
      "rev.1>out.1",
    ],
  ),
  preset("Empty", "Start from nothing", 112, [], []),
];
