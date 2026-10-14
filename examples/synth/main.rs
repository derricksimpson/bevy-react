//! **Synth** — a node graph editor for a modular synth, built with
//! bevy-react: patch the modules together and hear it.
//!
//!   * React owns the patch (`ui/src/graph/`): nodes, sockets, knobs, the
//!     palette, menus and every gesture.
//!   * Bevy owns what moves between them: the wires (`wires/`) — cables with
//!     a little physics, drawn under the UI by one shader with the dotted
//!     backdrop, signals flowing along them — the view's glide (`view.rs`),
//!     which keeps text crisp at any zoom, and the sound (`synth/`), the
//!     patch played sample by sample on the audio thread.
//!
//! Run (from the repo root):
//!
//!   npm install && npm run build -w synth
//!   cargo run -p synth

mod shoot;
mod spotlight;
mod synth;
mod view;
mod wires;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use bevy_react::prelude::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // `--export-bindings <path>` writes the typed TypeScript client
    // (`ui/src/bevy.ts`) from a bare App — no window, no JS runtime.
    if args.first().map(String::as_str) == Some("--export-bindings") {
        let path = args
            .get(1)
            .expect("--export-bindings requires an output path");
        let mut app = App::new();
        register_bindings(&mut app);
        app.export_react_typescript(path)
            .expect("failed to write TypeScript bindings");
        println!("wrote React bindings to {path}");
        return;
    }

    let shoot = shoot::parse(&args);
    let mut window = Window {
        title: "Synth · bevy-react".into(),
        resolution: WindowResolution::new(1440, 900),
        // The web build's canvas fills the page (`ui/index.html`).
        fit_canvas_to_parent: true,
        ..default()
    };
    if let Some(cfg) = &shoot {
        window.resolution = WindowResolution::new(cfg.size.0, cfg.size.1);
        // Uncapped, so the frame-time report measures the work, not vsync.
        window.present_mode = PresentMode::AutoNoVsync;
    }
    let mut app = build_app(window, shoot.is_some());
    if let Some(cfg) = shoot {
        shoot::install(&mut app, cfg);
    }
    app.run();
}

/// `shoot`: an offscreen capture — no hot reload, no sound.
fn build_app(window: Window, shoot: bool) -> App {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui/dist/app.js");
    let react = ReactUiPlugin::new(bundle)
        .hot_reload(!shoot)
        .default_font("fonts/SpaceGrotesk-VariableFont_wght.ttf")
        .font("Mono", "fonts/JetBrainsMonoNL-VariableFont_wght.ttf");

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(window),
        ..default()
    }))
    .add_plugins(ReactPlugins.set(react))
    .add_plugins((
        view::ViewPlugin,
        wires::WiresPlugin,
        spotlight::SpotlightPlugin,
        synth::SynthPlugin { audible: !shoot },
    ));
    app
}

/// Every React binding the app registers — the exporter's single source, so
/// the generated `bevy.ts` can't drift from the running app (each plugin
/// calls its own `register_bindings` in `build`).
fn register_bindings(app: &mut App) {
    ReactPlugins::register_bindings(app);
    view::register_bindings(app);
    wires::register_bindings(app);
    spotlight::register_bindings(app);
    synth::register_bindings(app);
    shoot::register_bindings(app);
}
