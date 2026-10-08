import type { Doc } from "../graph/model";
import { build, stops } from "./build";

export interface Preset {
  id: string;
  label: string;
  doc: () => Doc;
}

/** Weathered bricks: a running bond nudged by a soft noise, per-brick tint,
 *  grime in the mortar and on the faces. */
const bricks = () =>
  build(
    [
      [
        [
          "bricks",
          "bricks",
          { rows: 10, columns: 4, mortar: 5, bevel: 18, seed: 3 },
        ],
        ["soft", "noise", { scale: 5, detail: 3, seed: 11 }],
      ],
      [
        ["warp", "warp", { strength: 0.7 }],
        [
          "tint",
          "ramp",
          {
            ramp: stops(
              [0, [0.3, 0.12, 0.07]],
              [0.55, [0.5, 0.21, 0.11]],
              [1, [0.66, 0.36, 0.2]],
            ),
          },
        ],
      ],
      [
        ["grain", "noise", { scale: 14, detail: 7, roughness: 0.62 }],
        ["height", "mix", { mode: "multiply", fac: 0.22 }],
      ],
      [
        ["mortar", "mix", { a: [0.5, 0.48, 0.44] }],
        [
          "grime",
          "ramp",
          { ramp: stops([0, [0.45, 0.43, 0.4]], [0.7, [1, 1, 1]]) },
        ],
      ],
      [
        ["base", "mix", { mode: "multiply", fac: 0.55 }],
        ["normal", "normal", { strength: 5 }],
      ],
      [["rough", "mapRange", { toMin: 0.95, toMax: 0.74 }]],
      [["out", "output", { heightScale: 3 }]],
    ],
    [
      "bricks.mask > warp.image",
      "soft.fac > warp.intensity",
      "bricks.random > tint.fac",
      "warp.image > height.a",
      "grain.fac > height.b",
      "warp.image > mortar.fac",
      "tint.color > mortar.b",
      "grain.fac > grime.fac",
      "mortar.result > base.a",
      "grime.color > base.b",
      "height.result > normal.height",
      "warp.image > rough.value",
      "base.result > out.baseColor",
      "rough.result > out.roughness",
      "normal.normal > out.normal",
      "height.result > out.height",
    ],
  );

/** Riveted steel plates, rust eating in from a noise mask. */
const metal = () =>
  build(
    [
      [
        [
          "plates",
          "bricks",
          { rows: 2, columns: 2, offset: 0, mortar: 1.2, bevel: 2.5 },
        ],
        ["rivets", "shape", { tiles: 8, size: 0.16, softness: 0.5 }],
      ],
      [
        ["relief", "mix", { mode: "add", fac: 0.4 }],
        [
          "patches",
          "noise",
          { scale: 4, detail: 8, roughness: 0.6, distortion: 0.8, seed: 7 },
        ],
      ],
      [
        ["mask", "ramp", { ramp: stops([0.48, [0, 0, 0]], [0.6, [1, 1, 1]]) }],
        ["grain", "noise", { scale: 24, detail: 5, seed: 2 }],
      ],
      [
        [
          "rust",
          "ramp",
          {
            ramp: stops(
              [0, [0.2, 0.08, 0.04]],
              [0.5, [0.5, 0.21, 0.08]],
              [1, [0.72, 0.42, 0.18]],
            ),
          },
        ],
        ["steel", "color", { color: [0.62, 0.62, 0.64] }],
      ],
      [
        ["base", "mix", {}],
        ["metallic", "invert", {}],
      ],
      [
        ["rough", "mapRange", { toMin: 0.36, toMax: 0.9 }],
        ["normal", "normal", { strength: 4 }],
      ],
      [["out", "output", { heightScale: 2 }]],
    ],
    [
      "plates.mask > relief.a",
      "rivets.mask > relief.b",
      "patches.fac > mask.fac",
      "grain.fac > rust.fac",
      "mask.color > base.fac",
      "steel.color > base.a",
      "rust.color > base.b",
      "mask.color > metallic.color",
      "mask.color > rough.value",
      "relief.result > normal.height",
      "base.result > out.baseColor",
      "metallic.color > out.metallic",
      "rough.result > out.roughness",
      "normal.normal > out.normal",
      "relief.result > out.height",
    ],
  );

/** Cooled crust over glowing cracks: softly warped cells, an emission ramp. */
const lava = () =>
  build(
    [
      [
        ["cells", "voronoi", { feature: "edge", scale: 5 }],
        ["soft", "noise", { scale: 4, detail: 2, seed: 4 }],
      ],
      [
        ["warp", "warp", { strength: 1.4 }],
        [
          "cracks",
          "mapRange",
          { fromMax: 0.1, toMin: 1, toMax: 0, interpolation: "smoothstep" },
        ],
      ],
      [
        [
          "glow",
          "ramp",
          {
            ramp: stops(
              [0, [0, 0, 0]],
              [0.35, [0.45, 0.02, 0]],
              [0.7, [1, 0.32, 0.02]],
              [1, [1, 0.85, 0.4]],
            ),
          },
        ],
        ["grain", "noise", { scale: 10, detail: 7, roughness: 0.58 }],
      ],
      [
        [
          "rock",
          "ramp",
          { ramp: stops([0, [0.05, 0.045, 0.04]], [1, [0.24, 0.21, 0.19]]) },
        ],
        ["low", "invert", {}],
      ],
      [
        ["base", "mix", { b: [0.02, 0.02, 0.02] }],
        ["height", "mix", { mode: "multiply", fac: 0.3 }],
      ],
      [["normal", "normal", { strength: 7 }]],
      [
        [
          "out",
          "output",
          { emissionStrength: 6, roughness: 0.85, heightScale: 3 },
        ],
      ],
    ],
    [
      "cells.distance > warp.image",
      "soft.fac > warp.intensity",
      "warp.image > cracks.value",
      "cracks.result > glow.fac",
      "grain.fac > rock.fac",
      "cracks.result > base.fac",
      "rock.color > base.a",
      "cracks.result > low.color",
      "low.color > height.a",
      "grain.fac > height.b",
      "height.result > normal.height",
      "base.result > out.baseColor",
      "glow.color > out.emission",
      "normal.normal > out.normal",
      "height.result > out.height",
    ],
  );

export const PRESETS: Preset[] = [
  { id: "bricks", label: "Old Bricks", doc: bricks },
  { id: "metal", label: "Rusted Metal", doc: metal },
  { id: "lava", label: "Lava Rock", doc: lava },
];

/** A fresh graph: just the output. */
export const blank = () => build([[["out", "output", {}]]], []);
