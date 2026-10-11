//! The seventeen built-in filters, one file per shader family: [`color_matrix`]
//! (the seven color ops sharing one pass shader and packing — six declared by
//! the `color_matrix_filters!` macro plus the hand-written `hueRotate`),
//! [`blur`] (the two-pass separable Gaussian), [`bloom`] (bright-pass → blur
//! → combine, reusing blur's shader for its middle passes),
//! [`chromatic_aberration`] (single-pass directional RGB split),
//! [`gradient_map`] (multi-stop linear-gradient recolor), [`outline`]
//! (alpha-dilation ring under the content), [`shadow`] (offset + blurred
//! drop shadow, bloom's pass structure), [`pinch`] (cursor-anchored radial
//! pinch/bulge, all-normalized params), [`morph`] (`crossfade` and
//! `linearWipe`), and [`pixelize`] (the mosaic, a gl-transitions port). The
//! last three form the built-in **morph family** (`IS_MORPH`,
//! `morphFilter`-only — separate from the regular `filter`/`backdropFilter`
//! family). This module owns their registration; cross-family tests (the
//! shorthand-default and identity tables, WGSL validation) live here too.

mod bloom;
mod blur;
mod chromatic_aberration;
mod color_matrix;
mod gradient_map;
mod morph;
mod outline;
mod pinch;
mod pixelize;
mod shadow;

use bevy::prelude::*;

pub use bloom::BloomParams;
pub use blur::BlurParams;
pub use chromatic_aberration::ChromaticAberrationParams;
pub use color_matrix::{
    BrightnessParams, ContrastParams, GrayscaleParams, HueRotateParams, InvertParams,
    SaturateParams, SepiaParams,
};
pub use gradient_map::{GradientMapParams, GradientMapStop, MAX_GRADIENT_STOPS};
pub use morph::{CrossfadeParams, LinearWipeParams};
pub use outline::OutlineParams;
pub use pinch::PinchParams;
pub use pixelize::PixelizeParams;
pub use shadow::ShadowParams;

use super::registry::FilterRegistry;

impl FilterRegistry {
    /// Register the seventeen built-in filters into this registry. Two callers:
    /// [`register_builtin_filters`] (the runtime path, via
    /// `ReactUiPlugin::build`) and the TypeScript exporter
    /// (`crate::ts_codegen`), which seeds a throwaway registry with them so
    /// built-ins are always exported even though the bare exporter `App`
    /// (`register_bindings` only) never adds the plugin.
    pub(crate) fn register_builtins(&mut self) {
        self.register_builtin::<BloomParams>();
        self.register_builtin::<BlurParams>();
        self.register_builtin::<ChromaticAberrationParams>();
        self.register_builtin::<BrightnessParams>();
        self.register_builtin::<ContrastParams>();
        self.register_builtin::<SaturateParams>();
        self.register_builtin::<GrayscaleParams>();
        self.register_builtin::<SepiaParams>();
        self.register_builtin::<InvertParams>();
        self.register_builtin::<HueRotateParams>();
        self.register_builtin::<GradientMapParams>();
        self.register_builtin::<OutlineParams>();
        self.register_builtin::<PinchParams>();
        self.register_builtin::<ShadowParams>();
        self.register_builtin::<CrossfadeParams>();
        self.register_builtin::<LinearWipeParams>();
        self.register_builtin::<PixelizeParams>();
    }
}

/// Register the seventeen built-in filters. Called by `ReactUiPlugin::build`.
/// Deliberately `AssetServer`-free: shader loads happen lazily inside each
/// entry's `resolve`.
pub fn register_builtin_filters(app: &mut App) {
    app.world_mut()
        .get_resource_or_init::<FilterRegistry>()
        .register_builtins();
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::filters::params::MAX_FILTER_PARAM_VECS;
    use crate::filters::registry::ReactFilter;
    use crate::filters::test_util::{asset_app, builtin_registry, params};
    use crate::protocol::units::Length;

    /// `{}` params take each filter's shorthand default — the convention is a
    /// *visible* effect, not necessarily CSS's or the identity: 1.0 for the
    /// six amount ops (identity for brightness/contrast/saturate, full effect
    /// for grayscale/sepia/invert), 20px for blur radius (CSS's shorthand is
    /// 0 — the one deliberate divergence), 0 for hue angle.
    #[test]
    fn empty_params_take_visible_shorthand_defaults() {
        assert_eq!(params::<BrightnessParams>(json!({})).amount, 1.0);
        assert_eq!(params::<ContrastParams>(json!({})).amount, 1.0);
        assert_eq!(params::<SaturateParams>(json!({})).amount, 1.0);
        assert_eq!(params::<SepiaParams>(json!({})).amount, 1.0);
        assert_eq!(params::<InvertParams>(json!({})).amount, 1.0);
        // A bare `{name:"grayscale"}` APPLIES the effect: amount 1.0, packed
        // into its slot.
        let g = params::<GrayscaleParams>(json!({}));
        assert_eq!(g.amount, 1.0);
        assert_eq!(g.pack().0[0].w, 1.0);
        // A bare `{name:"blur"}` is a visible 20px blur (not CSS's 0).
        assert_eq!(params::<BlurParams>(json!({})).radius, Length::Px(20.0));
        assert_eq!(params::<HueRotateParams>(json!({})).angle.radians(), 0.0);
        // A bare `{name:"bloom"}` is a visible glow.
        let b = params::<BloomParams>(json!({}));
        assert_eq!(b.radius, Length::Px(12.0));
        assert_eq!(b.threshold, 0.7);
        assert_eq!(b.intensity, 1.0);
        // A bare `{name:"chromaticAberration"}` is visible fringing along +X.
        let ca = params::<ChromaticAberrationParams>(json!({}));
        assert_eq!(ca.offset, Length::Px(4.0));
        assert_eq!(ca.angle.radians(), 0.0);
        assert_eq!(ca.rotation, 0.0);
        // A bare `{name:"gradientMap"}` is a visible two-stop sweep.
        let gm = params::<GradientMapParams>(json!({}));
        assert_eq!(gm.stops.len(), 2);
        assert_eq!(gm.amount, 1.0);
        assert_eq!(gm.angle.radians(), 0.0);
        // A bare `{name:"outline"}` is a crisp 2px black outline.
        let o = params::<OutlineParams>(json!({}));
        assert_eq!(o.width, Length::Px(2.0));
        assert_eq!(o.softness, Length::Px(0.0));
        // A bare `{name:"pinch"}` is a visible squeeze at the center.
        let p = params::<PinchParams>(json!({}));
        assert_eq!((p.x, p.y), (0.5, 0.5));
        assert_eq!(p.strength, 0.5);
        assert_eq!(p.radius, 0.8);
        // A bare `{name:"shadow"}` is a soft black shadow below.
        let s = params::<ShadowParams>(json!({}));
        assert_eq!(s.offset_y, Length::Px(4.0));
        assert_eq!(s.spread, Length::Px(6.0));
        assert_eq!(s.color.0, [0.0, 0.0, 0.0, 0.6]);
    }

    /// Unknown param keys are rejected (deny-unknown-fields), both at the
    /// typed layer and through the registry's baked resolve fn.
    #[test]
    fn unknown_param_key_is_rejected() {
        assert!(serde_json::from_value::<BlurParams>(json!({ "radius": 4, "bogus": 1 })).is_err());
        let app = asset_app();
        let assets = app.world().resource::<AssetServer>();
        let mut registry = FilterRegistry::default();
        registry.register::<GrayscaleParams>();
        let err = (registry.entries["grayscale"].resolve)(&json!({ "typo": 2 }), assets)
            .expect_err("unknown key must reject");
        assert!(err.contains("grayscale"), "error names the filter: {err}");
    }

    /// Every built-in carries a TRUE identity — for grayscale/sepia/invert
    /// that is `0`, NOT their CSS shorthand default of `1` — and it resolves
    /// through the normal registry path to an identity packing.
    #[test]
    fn builtin_identity_params_are_true_identities() {
        let r = builtin_registry();
        let amount = |name: &str| (r.entries[name].identity)().unwrap()["amount"].clone();
        assert_eq!(amount("brightness"), json!(1.0));
        assert_eq!(amount("contrast"), json!(1.0));
        assert_eq!(amount("saturate"), json!(1.0));
        assert_eq!(amount("grayscale"), json!(0.0));
        assert_eq!(amount("sepia"), json!(0.0));
        assert_eq!(amount("invert"), json!(0.0));
        assert_eq!(
            (r.entries["blur"].identity)().unwrap()["radius"],
            json!(0.0)
        );
        assert_eq!(
            (r.entries["hueRotate"].identity)().unwrap()["angle"],
            json!(0.0)
        );
        assert_eq!(
            (r.entries["bloom"].identity)().unwrap()["intensity"],
            json!(0.0)
        );
        assert_eq!(
            (r.entries["chromaticAberration"].identity)().unwrap()["offset"],
            json!(0.0)
        );
        assert_eq!(
            (r.entries["gradientMap"].identity)().unwrap()["amount"],
            json!(0.0)
        );
        let outline_identity = (r.entries["outline"].identity)().unwrap();
        assert_eq!(outline_identity["width"], json!(0.0));
        assert_eq!(outline_identity["softness"], json!(0.0));
        assert_eq!(
            (r.entries["shadow"].identity)().unwrap()["color"],
            json!("transparent")
        );
        assert_eq!(
            (r.entries["pinch"].identity)().unwrap()["strength"],
            json!(0.0)
        );

        let app = asset_app();
        let assets = app.world().resource::<AssetServer>();
        let passes =
            (r.entries["grayscale"].resolve)(&(r.entries["grayscale"].identity)().unwrap(), assets)
                .expect("identity resolves");
        assert_eq!(passes[0].params[0].w, 0.0, "identity packing, no effect");

        // Bloom's identity resolves to the same four-pass structure (so
        // transitions stay `Aligned`) with intensity 0 packed in every pass —
        // the combine returns exactly the original regardless of
        // radius/threshold.
        let passes =
            (r.entries["bloom"].resolve)(&(r.entries["bloom"].identity)().unwrap(), assets)
                .expect("identity resolves");
        assert_eq!(passes.len(), 4);
        assert!(passes.iter().all(|p| p.params[1].y == 0.0));
    }

    /// The filter-pass shaders compose and validate: each is composed against
    /// the prelude by bevy's own `ShaderCache` — the WESL linker the GPU
    /// pipeline cache runs, here on the CPU — and the composed WGSL is parsed
    /// and validated by naga. A shader that fails here gates its layer on
    /// native and crashes the app on the web (wasm can't catch shader-module
    /// errors), so this is the pre-ship guard. Covers the built-ins and every
    /// example's prelude-importing `.wesl` (its filters and morphs).
    #[test]
    fn filter_shaders_compose_and_validate() {
        use bevy::asset::{AssetId, uuid::Uuid};
        use bevy::shader::{Shader, ShaderCache, ShaderCacheSource};

        const PRELUDE: &str = include_str!("../../layer/filter_prelude.wesl");

        /// Compose `src` (registered at asset `path`) against the prelude,
        /// parse + validate the result, and check its entry points (a pass
        /// the linker dropped would otherwise validate as the prelude alone).
        fn check(path: &str, src: &str, entry_points: &[&str]) {
            let mut cache = ShaderCache::<String, ()>::new((), |_, source, _| match source {
                ShaderCacheSource::Wgsl(wgsl) => Ok(wgsl),
                ShaderCacheSource::SpirV(_) => unreachable!("WESL composes to WGSL"),
            });
            let prelude = AssetId::Uuid {
                uuid: Uuid::from_u128(1),
            };
            cache.set_shader(
                prelude,
                Shader::from_wesl(PRELUDE, crate::plugin::FILTER_PRELUDE_PATH),
            );
            let id = AssetId::Uuid {
                uuid: Uuid::from_u128(2),
            };
            cache.set_shader(id, Shader::from_wesl(src.to_owned(), path.to_owned()));
            let composed = cache
                .get(0, id, &[])
                .unwrap_or_else(|e| panic!("{path} does not compose: {e}"));
            let module = naga::front::wgsl::parse_str(&composed).unwrap_or_else(|e| {
                panic!("{path} does not parse:\n{}", e.emit_to_string(&composed))
            });
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{path} does not validate: {e:?}"));
            for entry in entry_points {
                assert!(
                    module.entry_points.iter().any(|e| e.name == *entry),
                    "{path} is missing entry point `{entry}`"
                );
            }
        }

        check(crate::plugin::FILTER_PRELUDE_PATH, PRELUDE, &[]);
        for (name, src) in [
            ("color_matrix.wesl", include_str!("color_matrix.wesl")),
            ("blur.wesl", include_str!("blur.wesl")),
            ("bloom.wesl", include_str!("bloom.wesl")),
            (
                "chromatic_aberration.wesl",
                include_str!("chromatic_aberration.wesl"),
            ),
            ("gradient_map.wesl", include_str!("gradient_map.wesl")),
            ("outline.wesl", include_str!("outline.wesl")),
            ("shadow.wesl", include_str!("shadow.wesl")),
            ("pinch.wesl", include_str!("pinch.wesl")),
            ("crossfade.wesl", include_str!("crossfade.wesl")),
            ("linear_wipe.wesl", include_str!("linear_wipe.wesl")),
            ("pixelize.wesl", include_str!("pixelize.wesl")),
        ] {
            check(
                &format!("embedded://bevy_react_core/filters/builtin/{name}"),
                src,
                &["fragment"],
            );
        }

        // Every example's app-side passes (each `assets/shaders` `.wesl` that
        // imports the prelude) get the same check — their only pre-GPU
        // coverage. Skipped when the examples tree isn't present (a packaged
        // crate).
        let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut checked = 0;
        for assets in std::fs::read_dir(&examples)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path().join("assets"))
        {
            let mut dirs = vec![assets.join("shaders")];
            while let Some(dir) = dirs.pop() {
                for path in std::fs::read_dir(&dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                {
                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.extension().is_some_and(|e| e == "wesl")
                        && let Ok(src) = std::fs::read_to_string(&path)
                        && src.contains("import bevy_react::filter")
                    {
                        // The asset path, as `AssetServer::load` names it.
                        let asset = path.strip_prefix(&assets).unwrap().to_string_lossy();
                        check(&asset, &src, &["fragment"]);
                        checked += 1;
                    }
                }
            }
        }
        assert!(
            !examples.is_dir() || checked >= 3,
            "expected at least the demos' ripple/glitch/dissolve, checked {checked}"
        );

        // The prelude's params array must track `MAX_FILTER_PARAM_VECS`; a
        // mismatch would otherwise surface only at pipeline creation on a
        // live GPU (the filter-pass bind group in `layer/render.rs` vs. the
        // WGSL declaration).
        assert!(
            PRELUDE.contains(&format!("array<vec4<f32>, {MAX_FILTER_PARAM_VECS}>")),
            "filter_prelude.wesl params array size must equal MAX_FILTER_PARAM_VECS"
        );
    }
}
