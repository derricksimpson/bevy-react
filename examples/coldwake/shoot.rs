//! `--shoot <out.png> [settle-secs] [--size WxH] [--do "<secs> <action>"]…`
//! renders the app into an offscreen image (independent of window focus or
//! occlusion), plays the scripted actions, captures one PNG and exits.
//!
//! `--size` is the capture's size in pixels (default 1920×1080).
//!
//! Six actions are Bevy's own, since the player is: `go <spot>` stands him
//! somewhere (`layout::spot`: `start`, `center`, `store`, `bench`, `save`,
//! `hack`, `doors`, `lift`), `look <yaw> <pitch>` turns the free camera by
//! degrees from where the spot faces, `use` presses F (locks the camera on
//! the screen he stands before, or lets go), `point <u> <v>` puts the
//! cursor on the screen that holds the camera (0..1 from its top left),
//! `aim on|off` raises the gun, `walk <right> <ahead>` holds the walk keys
//! (`walk 0 1` = W, `walk 0 0` lets go). The rest are strings the React app
//! interprets (`debug.act` — see `useDebug` in `ui/src/hooks.ts`):
//! `menu on`, `tab <name>`, `select <item>`, `hit`, `fire`, `buy <item>`…
//! (the verbs are listed in the README). The clock starts when React's
//! first commit lands, so a slow isolate boot never swallows an early step.
//!
//! The real mouse and keys are dead here.

use std::path::PathBuf;

use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::input::mouse::{MouseButtonInput, MouseWheel};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy_react::{ReactAppExt, ReactEvents, ReactNode, react_event};

use crate::layout;
use crate::player::{MainCamera, Player, Scripted};

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
        settle_secs: 4.0,
        size: (1920, 1080),
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
    if let Some((at, action)) = cfg.steps.last()
        && *at > cfg.settle_secs
    {
        panic!(
            "--do \"{at} {action}\" comes after the capture at {}s",
            cfg.settle_secs
        );
    }
    Some(cfg)
}

#[derive(Resource)]
struct Shoot {
    cfg: ShootConfig,
    target: Option<ImageRenderTarget>,
    /// When React's first commit landed (the script clock's zero).
    start: Option<f32>,
    /// Frames and seconds since the first second, for the frame-time report.
    frames: (u32, f32),
    next_step: usize,
    shot: bool,
    captured: bool,
}

pub fn install(app: &mut App, cfg: ShootConfig) {
    app.init_resource::<Scripted>()
        .insert_resource(Shoot {
            cfg,
            target: None,
            start: None,
            frames: (0, 0.0),
            next_step: 0,
            shot: false,
            captured: false,
        })
        .add_systems(First, mute_mouse)
        .add_systems(PostStartup, redirect_camera)
        .add_systems(Update, drive);
}

/// The window is real and may sit under the user's pointer: their clicks
/// and wheel must never reach the capture.
fn mute_mouse(
    mut buttons: ResMut<Messages<MouseButtonInput>>,
    mut wheel: ResMut<Messages<MouseWheel>>,
) {
    buttons.clear();
    wheel.clear();
}

/// Point the main camera at an offscreen image.
fn redirect_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut shoot: ResMut<Shoot>,
    camera: Single<Entity, With<MainCamera>>,
) {
    let (w, h) = shoot.cfg.size;
    let handle = images.add(Image::new_target_texture(
        w,
        h,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let target = ImageRenderTarget {
        handle,
        scale_factor: 1.0,
    };
    commands
        .entity(*camera)
        .insert(RenderTarget::Image(target.clone()));
    shoot.target = Some(target);
}

/// One of Bevy's own steps (see the module docs); `false` = React's.
fn act(action: &str, player: &mut Player, scripted: &mut Scripted) -> bool {
    let mut words = action.split(' ');
    let (verb, a, b) = (words.next(), words.next(), words.next());
    let number = |word: Option<&str>| word.and_then(|w| w.parse::<f32>().ok());
    match (verb, a) {
        (Some("go"), Some(spot)) => {
            let (at, yaw) =
                layout::spot(spot).unwrap_or_else(|| panic!("--do: no spot named {spot:?}"));
            player.at = at;
            player.yaw = yaw;
            player.look = Vec2::new(yaw, 0.0);
        }
        (Some("look"), Some(_)) => {
            let (yaw, pitch) = (number(a).unwrap_or(0.0), number(b).unwrap_or(0.0));
            player.look = Vec2::new(player.yaw + yaw.to_radians(), pitch.to_radians());
        }
        (Some("aim"), Some(on)) => scripted.aiming = on == "on",
        (Some("use"), None) => scripted.toggle = true,
        (Some("point"), Some(_)) => {
            scripted.cursor = Some(Vec2::new(
                number(a).unwrap_or(0.5),
                number(b).unwrap_or(0.5),
            ));
        }
        (Some("walk"), Some(_)) => {
            scripted.wish = Vec2::new(number(a).unwrap_or(0.0), number(b).unwrap_or(0.0));
        }
        _ => return false,
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn drive(
    time: Res<Time>,
    mut shoot: ResMut<Shoot>,
    mut player: ResMut<Player>,
    mut scripted: ResMut<Scripted>,
    mounted: Query<(), With<ReactNode>>,
    events: ReactEvents,
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
        if !act(&action, &mut player, &mut scripted) {
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
