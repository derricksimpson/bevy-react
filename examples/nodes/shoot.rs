//! `--shoot <out.png> [settle-secs] [--size WxH] [--scale F] [--do "<secs> <action>"]…`
//! renders the app into an offscreen image (independent of window focus or
//! occlusion), plays the scripted actions, captures one PNG and exits.
//!
//! Actions are strings the React app interprets (`debug.act` — see
//! `useDebug` in `ui/src/hooks.ts`), e.g. `preset <name>`, `select <id>`,
//! `view <x> <y> <zoom>`, `menu add`, `mesh <name>`. The clock starts when
//! React's first commit lands, so a slow isolate boot never swallows an
//! early step.
//!
//! A few are Bevy's own, and play input on the capture: `ptr move <x> <y>`,
//! `ptr press <button>` and `ptr release <button>` drive a pointer (logical
//! px; button `0` left, `1` middle, `2` right), `type <text>` and
//! `key <Enter|Escape>` press keys.

use std::path::PathBuf;

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget, RenderTarget};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKeyCode};
use bevy::input::mouse::{MouseButtonInput, MouseWheel};
use bevy::picking::PickingSystems;
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::window::PrimaryWindow;
use bevy_react::{ReactAppExt, ReactEvents, ReactNode, react_event};

use crate::MainCamera;

/// Bevy → React: perform a scripted step (see the module docs).
#[react_event(name = "debug.act")]
pub struct Act {
    pub action: String,
}

pub fn register_bindings(app: &mut App) {
    app.add_react_event::<Act>();
}

pub struct ShootConfig {
    pub out: PathBuf,
    pub settle_secs: f32,
    pub size: (u32, u32),
    pub scale: f32,
    /// `(seconds after React's first commit, action)`, sorted.
    pub steps: Vec<(f32, String)>,
}

/// `None` unless the first argument is `--shoot`. Malformed input panics: a
/// silently ignored flag would produce a misleading capture.
pub fn parse(args: &[String]) -> Option<ShootConfig> {
    let mut it = args.iter();
    if it.next().map(String::as_str) != Some("--shoot") {
        return None;
    }
    let mut cfg = ShootConfig {
        out: PathBuf::new(),
        settle_secs: 3.0,
        size: (1440, 900),
        scale: 1.0,
        steps: Vec::new(),
    };
    let mut positional = Vec::new();
    while let Some(arg) = it.next() {
        let mut value = |flag: &str| it.next().unwrap_or_else(|| panic!("{flag} needs a value"));
        match arg.as_str() {
            "--size" => {
                let v = value("--size");
                let (w, h) = v
                    .split_once('x')
                    .unwrap_or_else(|| panic!("--size WxH, got {v}"));
                cfg.size = (w.parse().unwrap(), h.parse().unwrap());
            }
            "--scale" => cfg.scale = value("--scale").parse().expect("--scale F"),
            "--do" => {
                let v = value("--do");
                let (at, action) = v
                    .split_once(' ')
                    .unwrap_or_else(|| panic!("--do \"<secs> <action>\", got {v:?}"));
                cfg.steps
                    .push((at.parse().expect("--do secs"), action.to_string()));
            }
            flag if flag.starts_with("--") => panic!("unknown --shoot option {flag}"),
            _ => positional.push(arg.clone()),
        }
    }
    cfg.out = positional.first().expect("--shoot <out.png>").into();
    if let Some(secs) = positional.get(1) {
        cfg.settle_secs = secs.parse().expect("settle-secs");
    }
    cfg.steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some(cfg)
}

/// The scripted pointer (`ptr …` steps): the window's own cursor is never
/// over the capture.
const POINTER: PointerId = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(
    0x2f0c_91d4_7a3e_4b68_8c15_e6a0_b3d9_5172,
));

#[derive(Resource)]
struct Shoot {
    cfg: ShootConfig,
    target: Option<ImageRenderTarget>,
    /// Where the scripted pointer is (logical px).
    pointer: Vec2,
    /// When React's first commit landed (the script clock's zero).
    start: Option<f32>,
    /// Frames and seconds since the first second, for the frame-time report.
    frames: (u32, f32),
    next_step: usize,
    shot: bool,
    captured: bool,
}

pub fn install(app: &mut App, cfg: ShootConfig) {
    app.insert_resource(Shoot {
        cfg,
        target: None,
        pointer: Vec2::ZERO,
        start: None,
        frames: (0, 0.0),
        next_step: 0,
        shot: false,
        captured: false,
    })
    .add_systems(PostStartup, redirect_camera)
    .add_systems(First, mute_mouse.before(PickingSystems::Input))
    .add_systems(Update, drive);
}

/// The window is real and may sit under the user's pointer: their clicks
/// and wheel must never reach the capture (a stray wheel zooms the graph).
fn mute_mouse(
    mut buttons: ResMut<Messages<MouseButtonInput>>,
    mut wheel: ResMut<Messages<MouseWheel>>,
) {
    buttons.clear();
    wheel.clear();
}

/// Point the main camera (3D map + UI) at an offscreen image.
fn redirect_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut shoot: ResMut<Shoot>,
    camera: Single<Entity, With<MainCamera>>,
) {
    let (w, h) = shoot.cfg.size;
    let scale = shoot.cfg.scale;
    let handle = images.add(Image::new_target_texture(
        (w as f32 * scale).round() as u32,
        (h as f32 * scale).round() as u32,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let target = ImageRenderTarget {
        handle,
        scale_factor: scale,
    };
    commands
        .entity(*camera)
        .insert(RenderTarget::Image(target.clone()));
    commands.spawn(POINTER);
    shoot.target = Some(target);
}

/// Play one of Bevy's own steps (see the module docs); `false` when the
/// action is React's.
fn play(
    action: &str,
    shoot: &mut Shoot,
    window: Entity,
    pointer: &mut MessageWriter<PointerInput>,
    keys: &mut MessageWriter<KeyboardInput>,
) -> bool {
    let mut point = |shoot: &Shoot, action| {
        let location = Location {
            target: NormalizedRenderTarget::Image(shoot.target.clone().expect("redirected")),
            position: shoot.pointer,
        };
        pointer.write(PointerInput::new(POINTER, location, action));
    };
    let button = |button: &str| match button {
        "0" => PointerButton::Primary,
        "1" => PointerButton::Middle,
        "2" => PointerButton::Secondary,
        _ => panic!("a button is 0, 1 or 2, got {button:?}"),
    };
    let mut key = |key_code, logical_key: Key, text: Option<&str>| {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            keys.write(KeyboardInput {
                key_code,
                logical_key: logical_key.clone(),
                state,
                text: text.filter(|_| state.is_pressed()).map(Into::into),
                repeat: false,
                window,
            });
        }
    };
    if let Some(text) = action.strip_prefix("type ") {
        for c in text.chars() {
            let c = c.to_string();
            key(
                KeyCode::Unidentified(NativeKeyCode::Unidentified),
                Key::Character(c.as_str().into()),
                Some(&c),
            );
        }
        return true;
    }
    let words: Vec<&str> = action.split(' ').collect();
    match words[..] {
        ["ptr", "move", x, y] => {
            let to = Vec2::new(x.parse().expect("ptr x"), y.parse().expect("ptr y"));
            let delta = to - shoot.pointer;
            shoot.pointer = to;
            point(shoot, PointerAction::Move { delta });
        }
        ["ptr", "press", b] => point(shoot, PointerAction::Press(button(b))),
        ["ptr", "release", b] => point(shoot, PointerAction::Release(button(b))),
        ["key", "Enter"] => key(KeyCode::Enter, Key::Enter, None),
        ["key", "Escape"] => key(KeyCode::Escape, Key::Escape, None),
        _ => return false,
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn drive(
    time: Res<Time>,
    mut shoot: ResMut<Shoot>,
    mounted: Query<(), With<ReactNode>>,
    events: ReactEvents,
    window: Single<Entity, With<PrimaryWindow>>,
    mut pointer: MessageWriter<PointerInput>,
    mut keys: MessageWriter<KeyboardInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if shoot.captured {
        exit.write(AppExit::Success);
        return;
    }
    if shoot.shot {
        return;
    }
    let now = time.elapsed_secs();
    let start = match shoot.start {
        Some(start) => start,
        None if !mounted.is_empty() => *shoot.start.insert(now),
        None => return,
    };
    let t = now - start;
    // Past the first second (the isolate and pipeline warm-up).
    if t > 1.0 {
        shoot.frames.0 += 1;
        shoot.frames.1 += time.delta_secs();
    }
    while let Some((at, action)) = shoot.cfg.steps.get(shoot.next_step).cloned() {
        if at > t {
            break;
        }
        info!("shoot: {action} at {t:.2}s");
        if !play(&action, &mut shoot, *window, &mut pointer, &mut keys) {
            events.send(&Act { action });
        }
        shoot.next_step += 1;
    }
    if t < shoot.cfg.settle_secs {
        return;
    }
    let target = shoot.target.clone().expect("redirected in PostStartup");
    let (frames, secs) = shoot.frames;
    info!(
        "shoot: capturing → {} ({:.1} ms/frame over {frames} frames)",
        shoot.cfg.out.display(),
        secs * 1000.0 / frames.max(1) as f32
    );
    commands
        .spawn(Screenshot(RenderTarget::Image(target)))
        .observe(save_to_disk(shoot.cfg.out.clone()))
        .observe(|_: On<ScreenshotCaptured>, mut shoot: ResMut<Shoot>| {
            shoot.captured = true;
        });
    shoot.shot = true;
}
