//! You. The engineer walks the deck (W A S D, Shift to hurry) and the mouse
//! turns the camera over his right shoulder (`camera.rs`); he turns with
//! it. The right button aims, the left one fires.
//!
//! Before a screen, looking at it, F locks the camera on it (`lock.rs`):
//! he steps to his post beside it, the mouse moves a cursor over it instead
//! of turning the view, and the left button presses what the cursor is on
//! (`point` hands that ray to the `<surface>` pointer). F again, or a
//! step in any direction, gives the camera back. The suit menu holds the
//! camera the same way while it is up.
//!
//! The mouse is taken on a click and given back on Escape. React hears what
//! it has to show: `player.stance` (aiming, the mouse taken, the screen
//! that holds the camera, the one F would give it to) and `player.fire`.

mod camera;
pub mod lock;

use std::f32::consts::{PI, TAU};

use bevy::input::InputSystems;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_react::surface::{SurfacePointerRay, SurfacePointerSet};
use bevy_react::{ReactAppExt, ReactEvents, react_event, react_message};
use lock::{Screen, Shot};

pub use camera::MainCamera;

use crate::doors::Bulkhead;
use crate::holo::Requested;
use crate::layout::{self, FACING_LEFT, START};

/// Meters per second, walking and hurrying; aiming slows him.
pub const WALK: f32 = 2.16;
pub const RUN: f32 = 4.32;
const AIM_PACE: f32 = 0.55;
/// Radians per mouse count at the camera's widest view.
const SENSITIVITY: f32 = 0.0022;
const PITCH: (f32, f32) = (-0.6, 0.5);
/// On a locked screen a mouse count moves the cursor this many pixels of
/// the view.
const CURSOR_SPEED: f32 = 1.2;

/// Bevy → React: he raised or lowered the gun, the app took or gave back
/// the mouse, the camera locked on a screen (a hologram's name) or let go
/// of it, or he stands where F would lock it on one (`offer`).
#[react_event(name = "player.stance")]
pub struct StanceChanged {
    pub aiming: bool,
    pub mouse: bool,
    pub screen: Option<String>,
    pub offer: Option<String>,
}

/// Bevy → React: the trigger was pulled (React owns the clip).
#[react_event(name = "player.fire")]
pub struct FireShot;

/// React → Bevy: React (re)mounted and knows nothing: say everything again.
#[react_message(name = "player.sync")]
pub struct Sync;

/// Where he is and what he is doing.
#[derive(Resource)]
pub struct Player {
    /// His feet.
    pub at: Vec3,
    /// The way he faces (0 = -Z, growing to the left).
    pub yaw: f32,
    /// Where the camera looks: yaw, and pitch (+ = up).
    pub look: Vec2,
    pub velocity: Vec3,
    pub aiming: bool,
    /// How far into the aim he is, and into reading the suit menu (0..1).
    pub aim: f32,
    pub menu: f32,
    /// The app has taken the mouse.
    pub mouse: bool,
    /// The screen that holds the camera: a panel he stands at, or `menu`.
    pub held: Option<&'static str>,
    /// The panel F would give the camera to.
    pub offer: Option<&'static str>,
    /// The cursor on that screen: meters right of and above its center.
    pub cursor: Vec2,
    /// The way he turned to open the suit menu.
    menu_yaw: f32,
    /// The camera's field of view over its widest: the mouse slows with it.
    pub zoom: f32,
}

impl Player {
    /// His frame: +Y up, -Z ahead of him.
    pub fn frame(&self) -> Transform {
        Transform::from_translation(self.at).with_rotation(Quat::from_rotation_y(self.yaw))
    }

    /// The camera's rotation, which is also where he aims.
    pub fn view(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.look.x, self.look.y, 0.0)
    }

    /// The screen that holds the camera, and how a view `aspect` wide
    /// frames it.
    pub fn shot(&self, aspect: f32) -> Option<(Screen, Shot)> {
        let (screen, depth) = match self.held? {
            // The menu hangs on him: framed from where he stands.
            "menu" => {
                let menu = Screen::menu(self.at, self.menu_yaw);
                (menu, menu.before(self.at).y)
            }
            name => (lock::panel(name)?, lock::STAND),
        };
        Some((screen, screen.shot(depth, aspect, camera::WIDE)))
    }
}

/// `--shoot` plays him from a script: no mouse, no keys.
#[derive(Resource, Default)]
pub struct Scripted {
    pub aiming: bool,
    /// Where he walks: to his right, ahead (the W A S D axes).
    pub wish: Vec2,
    /// Where the cursor is on the screen that holds the camera (0..1 from
    /// its top left).
    pub cursor: Option<Vec2>,
    /// F is pressed this frame.
    pub toggle: bool,
}

/// What React was last told (`None` = nothing yet): aiming, the mouse, the
/// screen, the offer.
type Stance = (bool, bool, Option<&'static str>, Option<&'static str>);
#[derive(Resource, Default)]
struct Told(Option<Stance>);

/// Everything that moves him; pose and film after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerSet;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.insert_resource(Player {
            at: START,
            yaw: FACING_LEFT,
            look: Vec2::new(FACING_LEFT, 0.0),
            velocity: Vec3::ZERO,
            aiming: false,
            aim: 0.0,
            menu: 0.0,
            mouse: false,
            held: None,
            offer: None,
            cursor: Vec2::ZERO,
            menu_yaw: 0.0,
            zoom: 1.0,
        })
        .init_resource::<Told>()
        .add_systems(Startup, (camera::spawn_camera, take_mouse))
        // The cursor moves and points before the `<surface>` pointer is
        // driven: what is drawn, hovered and pressed is this frame's mouse.
        .add_systems(
            PreUpdate,
            point.after(InputSystems).before(SurfacePointerSet),
        )
        .add_systems(
            Update,
            (
                (grab, walk, report).chain().in_set(PlayerSet),
                camera::film.after(PlayerSet),
            ),
        );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_sync)
        .add_react_event::<StanceChanged>()
        .add_react_event::<FireShot>();
}

fn on_sync(_: On<Sync>, mut told: ResMut<Told>) {
    told.0 = None;
}

fn hold_mouse(cursor: &mut CursorOptions, on: bool) {
    cursor.grab_mode = if on {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !on;
}

/// A desktop window takes the mouse at once; a browser only lets a click.
fn take_mouse(
    scripted: Option<Res<Scripted>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if scripted.is_none() && !cfg!(target_arch = "wasm32") {
        hold_mouse(&mut cursor, true);
    }
}

/// Any click takes the mouse, Escape gives it back.
fn grab(
    scripted: Option<Res<Scripted>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if scripted.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        hold_mouse(&mut cursor, false);
    } else if buttons.get_just_pressed().next().is_some() {
        // Written on every click, also while taken: bevy_winit asks again,
        // and a browser may have dropped the lock on its own (its Escape
        // never reaches the page).
        hold_mouse(&mut cursor, true);
    }
}

/// The mouse moves the cursor of the screen that holds the camera, and the
/// ray from the camera through it goes to the `<surface>` pointer: the left
/// button and the wheel act where the cursor is.
fn point(
    scripted: Option<Res<Scripted>>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<&Transform, With<MainCamera>>,
    mut player: ResMut<Player>,
    mut pointer: ResMut<SurfacePointerRay>,
) {
    let aspect = window.width() / window.height().max(1.0);
    let held = player.shot(aspect).filter(|_| player.mouse);
    *pointer = SurfacePointerRay::Ray(held.and_then(|(screen, shot)| {
        // A count of the mouse is about a pixel of the view.
        let step = shot.width / window.width().max(1.0) * CURSOR_SPEED;
        let half = screen.size * 0.5;
        player.cursor = match scripted.as_ref().and_then(|script| script.cursor) {
            Some(uv) => Vec2::new(uv.x - 0.5, 0.5 - uv.y) * screen.size,
            None => {
                let moved = Vec2::new(motion.delta.x, -motion.delta.y) * step;
                (player.cursor + moved).clamp(-half, half)
            }
        };
        // From wherever the camera is: any ray through that point of the
        // screen lands on the same pixel of it.
        let to = screen.point(player.cursor) - camera.translation;
        Some(Ray3d::new(camera.translation, Dir3::new(to).ok()?))
    }));
}

/// Turn with the mouse, walk with the keys, raise the gun — or, held by a
/// screen, step to his post.
#[allow(clippy::too_many_arguments)]
fn walk(
    time: Res<Time>,
    mut scripted: Option<ResMut<Scripted>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<(&Window, &CursorOptions), With<PrimaryWindow>>,
    requested: Res<Requested>,
    bulkhead: Option<Res<Bulkhead>>,
    mut player: ResMut<Player>,
) {
    let dt = time.delta_secs();
    let (window, grab) = *window;
    let open = bulkhead.is_some_and(|b| b.is_open());
    let menu = requested.is_open("menu");
    let held = |key| keys.pressed(key) as i32 as f32;
    // Where he wants to go, whether he hurries, the mouse's travel, the
    // right button, F, a step begun. (A script's shots are taken as if the
    // mouse were his.)
    player.mouse = scripted.is_some() || grab.grab_mode != CursorGrabMode::None;
    let moves = [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD];
    let (mut wish, hurry, turn, trigger, toggle, stepped) = match &mut scripted {
        Some(script) => (
            script.wish,
            false,
            Vec2::ZERO,
            script.aiming,
            std::mem::take(&mut script.toggle),
            script.wish != Vec2::ZERO,
        ),
        None => (
            Vec2::new(
                held(KeyCode::KeyD) - held(KeyCode::KeyA),
                held(KeyCode::KeyW) - held(KeyCode::KeyS),
            )
            .normalize_or_zero(),
            keys.pressed(KeyCode::ShiftLeft),
            if player.mouse {
                motion.delta
            } else {
                Vec2::ZERO
            },
            player.mouse && buttons.pressed(MouseButton::Right),
            keys.just_pressed(KeyCode::KeyF),
            keys.any_just_pressed(moves),
        ),
    };

    // Which screen holds the camera: the menu while it is up; else the
    // panel F gave it to, until F again or a step; else nothing, and F
    // gives it to the panel he stands before (not with the gun raised).
    let was = player.held;
    let (sin, cos) = player.look.x.sin_cos();
    player.offer = lock::offer(player.at, Vec3::new(-sin, 0.0, -cos))
        .filter(|_| !menu && !trigger && was.is_none());
    player.held = match was.filter(|name| *name != "menu") {
        _ if menu => Some("menu"),
        Some(panel) => (!toggle && !stepped && lock::holds(panel, player.at)).then_some(panel),
        None => player.offer.filter(|_| toggle),
    };
    if player.held != was {
        player.cursor = Vec2::ZERO;
        match player.held {
            Some("menu") => player.menu_yaw = lock::menu_yaw(player.at, player.yaw, open),
            Some(_) => player.offer = None,
            // The camera comes back looking where he does.
            None => player.look = Vec2::new(player.yaw, 0.0),
        }
    }
    player.aiming = trigger && player.held.is_none();

    // The way he faces and walks: with the camera, or square to the screen.
    let aspect = window.width() / window.height().max(1.0);
    let shot = player.shot(aspect);
    let mut target = Vec3::ZERO;
    let face = if let Some((screen, shot)) = &shot {
        // (The mouse moves the screen's cursor: `point`.) He walks nowhere
        // of his own while a screen has him (a key still held from before F
        // doesn't count: a step is a new press).
        wish = Vec2::ZERO;
        if menu {
            player.menu_yaw
        } else {
            // He steps to his post beside the screen.
            let to_post = shot.post - player.at;
            if to_post.length() > 0.02 {
                target = (to_post * 5.0).clamp_length_max(WALK);
            }
            screen.facing()
        }
    } else {
        let turn = turn * SENSITIVITY * player.zoom;
        player.look.x -= turn.x;
        player.look.y = (player.look.y - turn.y).clamp(PITCH.0, PITCH.1);
        player.look.x
    };
    if wish != Vec2::ZERO {
        let (sin, cos) = face.sin_cos();
        let ahead = Vec3::new(-sin, 0.0, -cos);
        let right = Vec3::new(cos, 0.0, -sin);
        let pace = if hurry && !player.aiming { RUN } else { WALK };
        let pace = pace * if player.aiming { AIM_PACE } else { 1.0 };
        target = (right * wish.x + ahead * wish.y) * pace;
    }
    let velocity = player.velocity + (target - player.velocity) * (1.0 - (-10.0 * dt).exp());
    let to = layout::confine(player.at + velocity * dt, open);
    // What a wall stopped is not walked (the stride follows this).
    player.velocity = if dt > 0.0 {
        (to - player.at) / dt
    } else {
        Vec3::ZERO
    };
    player.at = to;

    // He turns the short way round.
    let turn = (face - player.yaw + PI).rem_euclid(TAU) - PI;
    player.yaw += turn * (1.0 - (-14.0 * dt).exp());
    let ease = 1.0 - (-9.0 * dt).exp();
    let (aiming, aim) = (player.aiming as i32 as f32, player.aim);
    player.aim += (aiming - aim) * ease;
    let reading = player.menu;
    player.menu += (menu as i32 as f32 - reading) * ease;
}

/// Tell React when the stance changes, and when the trigger is pulled.
fn report(
    player: Res<Player>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut told: ResMut<Told>,
    events: ReactEvents,
) {
    let now = (player.aiming, player.mouse, player.held, player.offer);
    if told.0 != Some(now) {
        told.0 = Some(now);
        events.send(&StanceChanged {
            aiming: now.0,
            mouse: now.1,
            screen: now.2.map(Into::into),
            offer: now.3.map(Into::into),
        });
    }
    if player.aiming && buttons.just_pressed(MouseButton::Left) {
        events.send(&FireShot);
    }
}
