/** Every hologram's surface and the logical px its React tree lays out in.
 *  Must match `HOLOS` in `holo/mod.rs` (a Rust test checks). */
export const SURFACES = {
  spine: { width: 48, height: 360 },
  stasis: { width: 160, height: 160 },
  air: { width: 220, height: 110 },
  ammo: { width: 220, height: 130 },
  menu: { width: 1280, height: 720 },
  store: { width: 1280, height: 760 },
  bench: { width: 1280, height: 760 },
  lock: { width: 200, height: 200 },
  save: { width: 480, height: 320 },
  elevator: { width: 240, height: 360 },
  floor: { width: 240, height: 80 },
  hack: { width: 420, height: 420 },
  title: { width: 960, height: 540 },
} as const;

export type SurfaceName = keyof typeof SURFACES;
