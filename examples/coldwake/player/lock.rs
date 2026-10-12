//! Locking the camera on a screen. Standing before a fixture's panel and
//! looking at it, F takes the camera off his shoulder and frames the panel
//! head-on: the panel against the right of the view, him stepped aside to a
//! post in what is left, his back at the frame's left edge. The mouse then
//! moves a cursor over the panel instead of turning the camera. F again, or
//! a step in any direction, gives the camera back.
//!
//! The suit menu is locked on the same way while it is up; it hangs on him,
//! so there is no post to step to.

use bevy::prelude::*;

use super::camera::WIDE;
use crate::holo::{HOLOS, Holo, Place};
use crate::layout;

/// Where the suit menu hangs from his feet, and the way it faces, in his
/// frame (-Z ahead of him): ahead and to his right, turned a little to the
/// camera. The engineer's anchor and the lock both read it.
pub const MENU_AT: Vec3 = Vec3::new(1.3, 1.55, -1.9);
pub const MENU_FACING: Vec3 = Vec3::new(-0.14, 0.0, 1.0);
/// The menu's far corners keep this far off the walls (the fixtures'
/// holograms hang a meter out from them).
const MENU_CLEARANCE: f32 = 1.15;

/// He stands this far in front of a panel he uses…
pub const STAND: f32 = 1.3;
/// …and the camera at least this far behind him.
const BEHIND: f32 = 1.1;
/// The screen takes this much of the view's width, or this much of its
/// height, whichever comes first, and stops this short of its right edge.
const SCREEN_WIDTH: f32 = 0.66;
const SCREEN_HEIGHT: f32 = 0.84;
const RIGHT_MARGIN: f32 = 0.05;
/// Half his width: he stands that far clear of the screen's left edge.
const SHOULDER: f32 = 0.36;
/// A panel offers itself this far short of its reach and this far to
/// either side of it; once it holds the camera it keeps it this much
/// farther out.
const SHORT_OF_REACH: f32 = 1.0;
const BESIDE: f32 = 0.7;
const SLACK: f32 = 0.4;
/// He has to look this squarely at a panel for it to offer itself (the
/// cosine of the angle off its face).
const FACING: f32 = 0.5;

/// A flat screen in the room: its center, the way it faces, its size
/// (meters).
#[derive(Clone, Copy)]
pub struct Screen {
    pub center: Vec3,
    pub normal: Vec3,
    pub size: Vec2,
}

impl Screen {
    /// A fixture's panel, where it hangs.
    fn of(holo: &Holo) -> Option<Self> {
        let Place::World(center, facing) = holo.place else {
            return None;
        };
        (holo.reach > 0.0).then(|| Self {
            center,
            normal: facing.normalize(),
            size: Vec2::new(holo.width_m, holo.width_m * holo.px.y / holo.px.x),
        })
    }

    /// The suit menu, hanging on him where he stands, facing `yaw`.
    pub fn menu(at: Vec3, yaw: f32) -> Self {
        let turn = Quat::from_rotation_y(yaw);
        let menu = HOLOS.iter().find(|holo| holo.name == "menu").unwrap();
        Self {
            center: at + turn * MENU_AT,
            normal: turn * MENU_FACING.normalize(),
            size: Vec2::new(menu.width_m, menu.width_m * menu.px.y / menu.px.x),
        }
    }

    /// To the right of someone looking at it.
    fn right(&self) -> Vec3 {
        (-self.normal).cross(Vec3::Y)
    }

    /// The yaw that looks at it square.
    pub fn facing(&self) -> f32 {
        f32::atan2(self.normal.x, self.normal.z)
    }

    /// A point on it: `cursor` meters right of and above its center.
    pub fn point(&self, cursor: Vec2) -> Vec3 {
        self.center + self.right() * cursor.x + Vec3::Y * cursor.y
    }

    /// How far to its right `at` is, and how far in front of it.
    pub fn before(&self, at: Vec3) -> Vec2 {
        let from = at - self.center;
        Vec2::new(from.dot(self.right()), from.dot(self.normal))
    }

    /// How the camera frames it for someone standing `depth` in front of
    /// it, in a view `aspect` wide and at most `wide` (radians) tall.
    pub fn shot(&self, depth: f32, aspect: f32, wide: f32) -> Shot {
        let width = (self.size.x / SCREEN_WIDTH).max(self.size.y * aspect / SCREEN_HEIGHT);
        // As near as the widest view shows all of that, but behind him.
        let distance = (width / (2.0 * (wide * 0.5).tan() * aspect)).max(depth + BEHIND);
        // The screen sits against the right of the view; the camera looks
        // at a point left of its center, and he stands left of its left
        // edge as the camera sees it past him.
        let aside = (0.5 - RIGHT_MARGIN) * width - self.size.x * 0.5;
        let edge = (aside - self.size.x * 0.5) * (distance - depth) / distance;
        let aim = self.center - self.right() * aside;
        Shot {
            aim,
            normal: self.normal,
            distance,
            width,
            post: (aim + self.normal * depth + self.right() * (edge - SHOULDER)).with_y(0.0),
        }
    }
}

/// A locked camera's framing of a screen: it looks at `aim` along the
/// screen's normal from `distance` out, and sees `width` meters across at
/// the screen. `post` is where he stands (his feet).
pub struct Shot {
    pub aim: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub width: f32,
    pub post: Vec3,
}

impl Shot {
    /// The camera's place, `distance` out (nearer if the room is short).
    pub fn eye(&self, distance: f32) -> Vec3 {
        self.aim + self.normal * distance
    }

    /// The vertical field of view that still shows `width` from `distance`.
    pub fn fov(&self, distance: f32, aspect: f32) -> f32 {
        2.0 * (self.width / (2.0 * distance * aspect)).atan()
    }
}

/// A fixture's panel by name, as a screen.
pub fn panel(name: &str) -> Option<Screen> {
    HOLOS
        .iter()
        .find(|holo| holo.name == name)
        .and_then(Screen::of)
}

/// Whether `at` is before `holo`'s panel, `slack` beyond where it offers
/// itself.
fn before(holo: &Holo, screen: &Screen, at: Vec3, slack: f32) -> bool {
    let before = screen.before(at);
    before.y > 0.0
        && before.y < holo.reach - SHORT_OF_REACH + slack
        && before.x.abs() < screen.size.x * 0.5 + BESIDE + slack
}

/// The panel F would lock the camera on: of those he stands before, looking
/// `ahead` at them, the one whose post is nearest.
pub fn offer(at: Vec3, ahead: Vec3) -> Option<&'static str> {
    HOLOS
        .iter()
        .filter_map(|holo| Some((holo, Screen::of(holo)?)))
        .filter(|(holo, screen)| {
            before(holo, screen, at, 0.0) && ahead.dot(-screen.normal) > FACING
        })
        .map(|(holo, screen)| {
            let post = screen.shot(STAND, 16.0 / 9.0, WIDE).post;
            (holo.name, post.distance(at))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(name, _)| name)
}

/// Whether the panel `name` can keep the camera on someone standing `at`
/// (not once he is somewhere else altogether).
pub fn holds(name: &str, at: Vec3) -> bool {
    HOLOS
        .iter()
        .filter(|holo| holo.name == name)
        .filter_map(|holo| Some((holo, Screen::of(holo)?)))
        .any(|(holo, screen)| before(holo, &screen, at, SLACK))
}

/// The way he turns to open the suit menu: the first of ahead, behind, left
/// and right with room in the air for it (else ahead).
pub fn menu_yaw(at: Vec3, yaw: f32, open: bool) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    [0.0, PI, FRAC_PI_2, -FRAC_PI_2]
        .into_iter()
        .map(|turn| yaw + turn)
        .find(|&yaw| {
            let menu = Screen::menu(at, yaw);
            let half = Vec2::new(menu.size.x * 0.5, 0.0);
            [half, -half]
                .into_iter()
                .all(|corner| layout::inside(menu.point(corner), MENU_CLEARANCE, open))
        })
        .unwrap_or(yaw)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASPECT: f32 = 16.0 / 9.0;

    fn panels() -> impl Iterator<Item = (&'static str, Screen)> {
        HOLOS
            .iter()
            .filter_map(|holo| Some((holo.name, Screen::of(holo)?)))
    }

    /// Every panel's post is somewhere he can stand and within the lock's
    /// hold, its camera is inside the room, and the view shows the whole
    /// screen with him left of it.
    #[test]
    fn every_panel_frames() {
        for (name, screen) in panels() {
            let shot = screen.shot(STAND, ASPECT, WIDE);
            assert_eq!(layout::confine(shot.post, false), shot.post, "{name}");
            assert!(holds(name, shot.post), "{name}");
            let eye = shot.eye(shot.distance);
            assert!(layout::inside(eye, 0.2, false), "{name}: camera at {eye}");
            let fov = shot.fov(shot.distance, ASPECT);
            assert!(fov <= WIDE + 1e-4, "{name}");
            // The screen's edges and his post, across the view (-1..1).
            let across = |p: Vec3| {
                let from = p - eye;
                from.dot(screen.right()) / (-from.dot(screen.normal) * (fov * 0.5).tan() * ASPECT)
            };
            let half = Vec2::new(screen.size.x * 0.5, 0.0);
            assert!(across(screen.point(half)) < 0.95, "{name}");
            let left = across(screen.point(-half));
            assert!(left > -1.0, "{name}");
            assert!(across(shot.post) < left, "{name}");
        }
    }

    /// A panel offers itself to whoever stands before it looking at it, not
    /// to someone walking past, and lets go of someone who is elsewhere.
    #[test]
    fn a_panel_offers_itself_to_whoever_faces_it() {
        let store = panel("store").unwrap();
        let before = store.center.with_y(0.0) + store.normal * 2.0;
        assert_eq!(offer(before, -store.normal), Some("store"));
        // Along the wall, looking down the room.
        assert_eq!(offer(before, Vec3::NEG_Z), None);
        assert!(holds("store", before));
        let away = store.center.with_y(0.0) + store.normal * 3.5;
        assert_eq!(offer(away, -store.normal), None);
        assert!(!holds("store", away));
    }

    /// Between two panels of the far wall, the one whose post is nearer.
    #[test]
    fn the_nearer_post_wins() {
        let post = |name| panel(name).unwrap().shot(STAND, ASPECT, WIDE).post;
        assert_eq!(offer(post("hack"), Vec3::NEG_Z), Some("hack"));
        assert_eq!(offer(post("save"), Vec3::NEG_Z), Some("save"));
    }

    /// He turns his back to a wall to open the menu into the room.
    #[test]
    fn the_menu_opens_into_the_room() {
        let at = Vec3::new(0.0, 0.0, 1.0);
        assert_eq!(menu_yaw(at, 0.0, false), 0.0);
        let by_the_wall = Vec3::new(-2.5, 0.0, -3.7);
        assert_eq!(menu_yaw(by_the_wall, 0.0, false), std::f32::consts::PI);
    }
}
