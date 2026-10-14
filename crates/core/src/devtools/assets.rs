//! The panel's own assets, both served from inside the crate so any app
//! gets them:
//!
//! - its icons: 24×24 stroke glyphs (Lucide's style), drawn white so the
//!   panel's `<image tint>` colors them, embedded svg assets at
//!   `embedded://bevy_react_core/devtools/icons/<name>.svg` (`Icon.tsx`). An
//!   svg-mode `<image>` is core, so the panel needs no `svg` feature, and
//!   each icon rasters crisp at its laid-out size;
//! - its monospace face: Bevy's built-in Fira Mono (the core enables
//!   `default_font`) under [`MONO_FONT`] — a system font is not reachable by
//!   name, and the app's own fonts are the app's.

use std::path::{Path, PathBuf};

use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::prelude::*;

use crate::plugin::Fonts;

/// The `fontFamily` of the panel's monospace text (`theme.ts`'s `mono`).
const MONO_FONT: &str = "bevy-react devtools mono";

/// `(name, svg body)`; the names are `Icon.tsx`'s `IconName`. A filled shape
/// sets its own `fill` (a translucent one reads as the "panel" area).
const ICONS: &[(&str, &str)] = &[
    (
        "dock",
        r##"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4z" fill="#fff" fill-opacity=".45" stroke="none"/><path d="M15 3v18"/>"##,
    ),
    (
        "float",
        r##"<path d="M21 10V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h4"/><rect x="13" y="14" width="9" height="7" rx="1.5" fill="#fff" fill-opacity=".45"/>"##,
    ),
    (
        "push",
        r##"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4z" fill="#fff" fill-opacity=".45" stroke="none"/><path d="M15 3v18M11 12H6m3-3-3 3 3 3"/>"##,
    ),
    (
        "pick",
        r##"<path d="M12.03 12.68a.5.5 0 0 1 .65-.65l9 3.5a.5.5 0 0 1-.03.94l-3.45 1.07a1 1 0 0 0-.66.66l-1.07 3.44a.5.5 0 0 1-.94.03z"/><path d="M5 3a2 2 0 0 0-2 2M19 3a2 2 0 0 1 2 2M5 21a2 2 0 0 1-2-2M9 3h1M9 21h2M14 3h1M3 9v1M21 9v2M3 14v1"/>"##,
    ),
    (
        "overlay",
        r##"<path d="M3 7V5a2 2 0 0 1 2-2h2M17 3h2a2 2 0 0 1 2 2v2M21 17v2a2 2 0 0 1-2 2h-2M7 21H5a2 2 0 0 1-2-2v-2"/><rect x="7.5" y="7.5" width="9" height="9" rx="1" fill="#fff" fill-opacity=".45"/>"##,
    ),
    (
        "paint",
        r##"<path d="m9.06 11.9 8.07-8.06a2.85 2.85 0 1 1 4.03 4.03l-8.06 8.08"/><path d="M7.07 14.94c-1.66 0-3 1.35-3 3.02 0 1.33-2.5 1.52-2 2.02 1.08 1.1 2.49 2.02 4 2.02 2.2 0 4-1.8 4-4.04a3.01 3.01 0 0 0-3-3.02z"/>"##,
    ),
    ("close", r##"<path d="M18 6 6 18M6 6l12 12"/>"##),
    (
        "nodes",
        r##"<path d="M21 12h-8M21 6H8M21 18h-8M3 6v4c0 1.1.9 2 2 2h3M3 10v6c0 1.1.9 2 2 2h3"/>"##,
    ),
    (
        "layers",
        r##"<path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65M22 12.65l-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>"##,
    ),
    (
        "animations",
        r##"<circle cx="19" cy="5" r="2"/><circle cx="5" cy="19" r="2"/><path d="M5 17A12 12 0 0 1 17 5"/>"##,
    ),
    ("console", r##"<path d="m4 17 6-6-6-6M12 19h8"/>"##),
    (
        "bridge",
        r##"<path d="M8 3 4 7l4 4M4 7h16M16 21l4-4-4-4M20 17H4"/>"##,
    ),
    (
        "pause",
        r##"<rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/>"##,
    ),
    ("play", r##"<path d="M6 3l14 9-14 9z"/>"##),
    (
        "clear",
        r##"<circle cx="12" cy="12" r="10"/><path d="m4.9 4.9 14.2 14.2"/>"##,
    ),
    ("expandAll", r##"<path d="m7 15 5 5 5-5M7 9l5-5 5 5"/>"##),
    ("collapseAll", r##"<path d="m7 20 5-5 5 5M7 4l5 5 5-5"/>"##),
    (
        "check",
        r##"<path d="M20 6 9 17l-5-5" stroke-width="3.5"/>"##,
    ),
    (
        "warning",
        r##"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3z"/><path d="M12 9v4M12 17h.01"/>"##,
    ),
];

/// Serve [`ICONS`] from the embedded asset source, and name [`MONO_FONT`]
/// once the core has loaded the app's fonts (`Startup`). Icons are skipped
/// without an asset source (a headless harness with no `AssetPlugin` draws
/// no panel).
pub(super) fn register(app: &mut App) {
    app.add_systems(PostStartup, name_mono_font);
    let Some(registry) = app.world().get_resource::<EmbeddedAssetRegistry>() else {
        return;
    };
    for (name, body) in ICONS {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"##
        );
        let path = format!("bevy_react_core/devtools/icons/{name}.svg");
        registry.insert_asset(PathBuf::new(), Path::new(&path), svg.into_bytes());
    }
}

fn name_mono_font(fonts: Option<ResMut<Fonts>>) {
    if let Some(mut fonts) = fonts {
        // The default handle is the built-in face.
        fonts.named.insert(MONO_FONT.into(), Handle::default());
    }
}

/// `Icon.tsx`'s `IconName` union and [`ICONS`] name the same set: a name the
/// panel asks for and Rust lacks would load nothing.
#[test]
fn icon_names_match_the_panel() {
    let tsx = include_str!("../../../../js/src/devtools/Icon.tsx");
    let union = tsx
        .split("export type IconName =")
        .nth(1)
        .and_then(|rest| rest.split(';').next())
        .expect("Icon.tsx declares `export type IconName = …;`");
    let mut ts: Vec<&str> = union.split('"').skip(1).step_by(2).collect();
    let mut rust: Vec<&str> = ICONS.iter().map(|(name, _)| *name).collect();
    ts.sort_unstable();
    rust.sort_unstable();
    assert_eq!(ts, rust);
}

/// `theme.ts` asks for the face by the name Rust registers.
#[test]
fn mono_font_name_matches_the_theme() {
    let theme = include_str!("../../../../js/src/devtools/theme.ts");
    assert!(theme.contains(&format!("mono: \"{MONO_FONT}\"")));
}
