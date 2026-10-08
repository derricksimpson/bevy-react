//! **Nodes** — a node editor for procedural textures, built with bevy-react.
//!
//! The editor is React: the graph (pan, zoom, drag nodes and wires, box
//! select, the Add menu, undo), every field inside the nodes, the menus and
//! the side panel. Bevy evaluates the graph on the CPU, shows a live
//! thumbnail on every node, and renders the result as a PBR material in the
//! 3D preview (a `<portal>`). The wires and the grid are two elements the app
//! registers itself (`wire.rs`).
//!
//! Run (from the repo root):
//!
//!   npm install && npm run build -w nodes
//!   cargo run -p nodes

mod graph;
mod preview;
mod shoot;
mod wire;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use bevy_react::prelude::*;

/// The window behind the UI (only seen before the first frame).
const BACKGROUND: Color = Color::srgb(0.11, 0.11, 0.11);

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
        title: "Nodes · bevy-react".into(),
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
    let mut app = build_app(window, shoot.is_none());
    if let Some(cfg) = shoot {
        shoot::install(&mut app, cfg);
    }
    app.run();
}

fn build_app(window: Window, hot_reload: bool) -> App {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui/dist/app.js");
    let react = ReactUiPlugin::new(bundle)
        .hot_reload(hot_reload)
        .default_font("fonts/Inter-VariableFont_opsz,wght.ttf");

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(window),
        ..default()
    }))
    .add_plugins(ReactPlugins.set(react))
    .add_plugins((graph::GraphPlugin, preview::PreviewPlugin, wire::WirePlugin))
    .add_systems(Startup, spawn_camera);
    shoot::register_bindings(&mut app);
    app
}

/// The UI camera (the 3D preview has its own, see `preview.rs`).
#[derive(Component)]
pub struct MainCamera;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(BACKGROUND),
            ..default()
        },
        IsDefaultUiCamera,
        MainCamera,
    ));
}

/// Every React binding the app registers — the exporter's single source, so
/// the generated `bevy.ts` can't drift from the running app (each plugin
/// calls its own `register_bindings` in `build`).
fn register_bindings(app: &mut App) {
    ReactPlugins::register_bindings(app);
    graph::register_bindings(app);
    preview::register_bindings(app);
    wire::register_bindings(app);
    shoot::register_bindings(app);
}
