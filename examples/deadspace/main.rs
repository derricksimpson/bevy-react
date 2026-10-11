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
//!   * the holograms: where each one hangs, how it unfolds and the WGSL that
//!     draws it (`holo/`);
//!   * the camera tour (`tour.rs`), the doors and the elevator (`doors.rs`),
//!     the locator line (`locator.rs`), the item preview and the map
//!     (`preview/`), the synthesized sound (`sound/`).
//!
//! Run (from the repo root):
//!
//!   npm install && npm run build -w deadspace
//!   cargo run -p deadspace

mod doors;
mod engineer;
mod filters;
mod holo;
mod layout;
mod locator;
mod preview;
mod shoot;
mod sound;
mod station;
mod tour;
mod weapons;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow, WindowResolution};
use bevy_react::prelude::*;

/// The flat UI (the beat strip) is laid out in 1080p logical pixels whatever
/// the window size: the window's scale factor follows its height
/// (`fit_ui_to_1080p`).
pub const UI_HEIGHT: f32 = 1080.0;

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
        // Starts at the scale `fit_ui_to_1080p` keeps (no first-frame jump).
        resolution: WindowResolution::new(1600, 900).with_scale_factor_override(900.0 / UI_HEIGHT),
        // The web build's canvas fills the page (`ui/index.html`).
        fit_canvas_to_parent: true,
        ..default()
    };
    if let Some(cfg) = &shoot {
        window.resolution = WindowResolution::new(cfg.size.0, cfg.size.1);
        // Uncapped, so the frame-time report measures the work, not vsync.
        window.present_mode = PresentMode::AutoNoVsync;
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
        tour::TourPlugin,
        doors::DoorsPlugin,
        locator::LocatorPlugin,
        preview::PreviewPlugin,
        sound::SoundPlugin,
    ))
    .add_systems(Update, fit_ui_to_1080p);
    filters::register_bindings(&mut app);
    register_app_bindings(&mut app);
    app
}

/// Every React binding the app registers — the exporter's single source, so
/// the generated `bevy.ts` can't drift from the running app (each plugin
/// calls its own `register_bindings` in `build`).
fn register_bindings(app: &mut App) {
    ReactPlugins::register_bindings(app);
    filters::register_bindings(app);
    register_app_bindings(app);
    engineer::register_bindings(app);
    weapons::register_bindings(app);
    holo::register_bindings(app);
    tour::register_bindings(app);
    doors::register_bindings(app);
    locator::register_bindings(app);
    preview::register_bindings(app);
    sound::register_bindings(app);
    shoot::register_bindings(app);
}

/// React → Bevy: QUIT on the title terminal.
#[react_message(name = "app.quit")]
pub struct Quit;

fn register_app_bindings(app: &mut App) {
    app.add_react_handler(quit);
}

fn quit(_: On<Quit>, mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}

/// Keep the window's logical height at [`UI_HEIGHT`] (see cyberpunk's
/// twin): the flat UI keeps its proportions at any resolution.
/// Compare-before-write, so a settled window is never touched.
fn fit_ui_to_1080p(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        let height = window.resolution.physical_height();
        if height == 0 {
            continue;
        }
        let scale = height as f32 / UI_HEIGHT;
        if window
            .resolution
            .scale_factor_override()
            .is_none_or(|s| (s - scale).abs() > 1e-4)
        {
            window.resolution.set_scale_factor_override(Some(scale));
        }
    }
}
