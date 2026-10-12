//! **COLDWAKE** — a homage to the diegetic interface of a certain 2011
//! sci-fi horror game, built with bevy-react: every readout lives in the
//! world. The engineer's health runs down his spine, the stasis gauge sits
//! on his shoulder blade, the ammo count floats over the gun, and the menus,
//! the store and the bench are holograms projected into the room.
//!
//! Every hologram is a React `<surface>` on a mesh. React owns the fake game
//! (`ui/src/game/`); Bevy owns everything you see it on:
//!
//!   * the deck, its lights, fog, steam and dust (`station/`);
//!   * the engineer and his poses, the weapons and their shots (`engineer/`,
//!     `weapons/`);
//!   * the holograms: where each one hangs, how it unfolds and the WESL that
//!     draws it (`holo/`);
//!   * you: walking him, the camera over his shoulder, the pointer in the
//!     middle of the screen (`player/`);
//!   * the doors and the elevator (`doors/`), the locator line
//!     (`locator.rs`), the item preview and the map (`preview/`), the
//!     synthesized sound (`sound/`).
//!
//! Run (from the repo root):
//!
//!   npm install && npm run build -w coldwake
//!   cargo run -p coldwake

mod doors;
mod engineer;
mod filters;
mod holo;
mod layout;
mod locator;
mod player;
mod preview;
mod shoot;
mod sound;
mod station;
mod weapons;

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
        title: "COLDWAKE · bevy-react".into(),
        resolution: WindowResolution::new(1600, 900),
        // The web build's canvas fills the page (`ui/index.html`).
        fit_canvas_to_parent: true,
        // No vsync: the cursor on a locked screen is drawn by the app, and
        // a frame held for the display's refresh is a frame it trails the
        // mouse (it costs a GPU running flat out, and tearing where the
        // system doesn't composite). It also lets `--shoot`'s frame-time
        // report measure the work.
        present_mode: PresentMode::AutoNoVsync,
        // Where the system keeps vsync anyway, one frame queued for the
        // display, not wgpu's two.
        desired_maximum_frame_latency: std::num::NonZero::new(1),
        ..default()
    };
    if let Some(cfg) = &shoot {
        window.resolution = WindowResolution::new(cfg.size.0, cfg.size.1);
    }
    let mut app = build_app(window, shoot.is_none());
    if let Some(cfg) = shoot {
        shoot::install(&mut app, cfg);
    }
    app.run();
}

fn build_app(window: Window, hot_reload: bool) -> App {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui/dist/app.js");
    // Saira ships as static weights, so each one is its own family.
    let react = ReactUiPlugin::new(bundle)
        .hot_reload(hot_reload)
        .default_font("fonts/SairaSemiCondensed-Medium.ttf")
        .font("Saira", "fonts/SairaSemiCondensed-Regular.ttf")
        .font("Saira SemiBold", "fonts/SairaSemiCondensed-SemiBold.ttf")
        .font("Saira Bold", "fonts/SairaSemiCondensed-Bold.ttf")
        .font("Michroma", "fonts/Michroma-Regular.ttf");

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(window),
        ..default()
    }))
    .add_plugins(ReactPlugins.set(react))
    .add_plugins((
        station::StationPlugin,
        engineer::EngineerPlugin,
        weapons::WeaponsPlugin,
        holo::HoloPlugin,
        player::PlayerPlugin,
        doors::DoorsPlugin,
        locator::LocatorPlugin,
        preview::PreviewPlugin,
        sound::SoundPlugin,
    ));
    filters::register_bindings(&mut app);
    app
}

/// Every React binding the app registers — the exporter's single source, so
/// the generated `bevy.ts` can't drift from the running app (each plugin
/// calls its own `register_bindings` in `build`).
fn register_bindings(app: &mut App) {
    ReactPlugins::register_bindings(app);
    filters::register_bindings(app);
    engineer::register_bindings(app);
    weapons::register_bindings(app);
    holo::register_bindings(app);
    player::register_bindings(app);
    doors::register_bindings(app);
    locator::register_bindings(app);
    preview::register_bindings(app);
    sound::register_bindings(app);
    shoot::register_bindings(app);
}
