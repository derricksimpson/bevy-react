//! Posing the suit every frame. A pose is a handful of numbers — where the
//! hips sit, how the spine, chest and head turn, where the gun is, where
//! the feet stand — and what the player does picks which pose he eases
//! toward (idle, aiming, reading the menu). Walking strides the feet along
//! the way he goes; reactions add on top (a flinch, a stasis throw, the
//! gun's kick). The arms and legs are solved with two-bone IK: hands on
//! the gun's grips (or hanging), feet on the floor, so every pose and blend
//! stays connected.

use bevy::prelude::*;

use super::suit::{
    ANKLE_HEIGHT, CHEST, FOREARM, HIP, HIPS, NECK, Rig, SHIN, SHOULDER, SPINE, THIGH, UPPER_ARM,
};
use crate::player::{Player, RUN, WALK};
use crate::station::Noise;

/// Where the hands hold a weapon, in the gun's frame (the weapon models
/// put their grips here).
pub const GRIP: Vec3 = Vec3::new(0.0, -0.06, 0.06);
pub const FOREGRIP: Vec3 = Vec3::new(0.0, -0.05, -0.14);

/// What the weapons write: the gun's kick (1 on a shot, fading) and the
/// reload's progress (0..1 while reloading).
#[derive(Resource, Default)]
pub struct GunKick {
    pub recoil: f32,
    pub reload: Option<f32>,
}

/// The suit's own reactions, fading: a flinch (with its lean), a stasis
/// throw with the left hand.
#[derive(Resource, Default)]
pub struct Reactions {
    pub flinch: f32,
    pub flinch_roll: f32,
    pub throw: Option<f32>,
}

#[derive(Clone, Copy)]
struct Pose {
    /// Hips offset from rest, and their turn (pitch, yaw, roll).
    hips: Vec3,
    hips_rot: Vec3,
    spine: Vec3,
    chest: Vec3,
    head: Vec3,
    /// The gun's place and turn, in the root's frame.
    gun: Vec3,
    gun_rot: Vec3,
    /// 0 = the left hand hangs, 1 = it holds the foregrip.
    grip: f32,
    /// Ankle targets, right then left.
    feet: [Vec3; 2],
}

impl Pose {
    fn lerp(&self, o: &Pose, k: f32) -> Pose {
        Pose {
            hips: self.hips.lerp(o.hips, k),
            hips_rot: self.hips_rot.lerp(o.hips_rot, k),
            spine: self.spine.lerp(o.spine, k),
            chest: self.chest.lerp(o.chest, k),
            head: self.head.lerp(o.head, k),
            gun: self.gun.lerp(o.gun, k),
            gun_rot: self.gun_rot.lerp(o.gun_rot, k),
            grip: self.grip + (o.grip - self.grip) * k,
            feet: [
                self.feet[0].lerp(o.feet[0], k),
                self.feet[1].lerp(o.feet[1], k),
            ],
        }
    }
}

/// Rotations are (pitch, yaw, roll) radians: +pitch leans back (looks up),
/// +yaw turns left, −yaw right.
const IDLE: Pose = Pose {
    hips: Vec3::ZERO,
    hips_rot: Vec3::new(0.0, 0.04, 0.0),
    spine: Vec3::new(-0.02, 0.0, 0.0),
    chest: Vec3::new(-0.03, 0.04, 0.0),
    head: Vec3::new(-0.06, -0.05, 0.0),
    gun: Vec3::new(0.26, 0.92, -0.16),
    gun_rot: Vec3::new(-1.05, -0.12, 0.0),
    grip: 0.0,
    feet: [
        Vec3::new(0.14, ANKLE_HEIGHT, 0.04),
        Vec3::new(-0.13, ANKLE_HEIGHT, -0.08),
    ],
};

const AIM: Pose = Pose {
    hips: Vec3::new(0.02, -0.05, 0.03),
    hips_rot: Vec3::new(0.0, -0.22, 0.0),
    spine: Vec3::new(-0.1, -0.1, 0.0),
    chest: Vec3::new(-0.04, -0.14, 0.0),
    head: Vec3::new(-0.06, 0.42, 0.0),
    gun: Vec3::new(0.17, 1.44, -0.34),
    gun_rot: Vec3::ZERO,
    grip: 1.0,
    feet: [
        Vec3::new(0.2, ANKLE_HEIGHT, 0.14),
        Vec3::new(-0.15, ANKLE_HEIGHT, -0.2),
    ],
};

const MENU: Pose = Pose {
    hips: Vec3::ZERO,
    hips_rot: Vec3::new(0.0, -0.04, 0.0),
    spine: Vec3::new(0.0, 0.0, 0.0),
    chest: Vec3::new(0.0, -0.08, 0.0),
    head: Vec3::new(-0.1, -0.12, 0.0),
    ..IDLE
};

/// His stride: where in the cycle his feet are (radians; the right foot
/// lands at every whole turn, the left half a turn later) and how much of
/// it shows (0 standing .. 1 walking).
#[derive(Default)]
pub struct Stride {
    phase: f32,
    amount: f32,
}

fn euler(r: Vec3) -> Quat {
    Quat::from_euler(EulerRot::YXZ, r.y, r.x, r.z)
}

/// The elbow (or knee) of a two-bone chain from `s` reaching for `t`,
/// bending toward `pole`. Clamped to reach.
fn two_bone(s: Vec3, t: Vec3, a: f32, b: f32, pole: Vec3) -> (Vec3, Vec3) {
    let d = (t - s).length().clamp((a - b).abs() + 1e-3, a + b - 1e-3);
    let dir = (t - s).normalize_or(Vec3::NEG_Y);
    let along = (a * a - b * b + d * d) / (2.0 * d);
    let h = (a * a - along * along).max(0.0).sqrt();
    let side = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Z);
    let mid = s + dir * along + side * h;
    (mid, s + dir * d)
}

/// The rotation turning a limb (rest: hanging along −Y, its back +Z) to run
/// from `a` to `b`, its back toward `back`.
fn limb(a: Vec3, b: Vec3, back: Vec3) -> Quat {
    let y = (a - b).normalize_or(Vec3::Y);
    let z = (back - y * back.dot(y)).normalize_or(Vec3::Z);
    let x = y.cross(z).normalize();
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

#[allow(clippy::too_many_arguments)]
pub fn pose(
    time: Res<Time>,
    player: Res<Player>,
    mut stride: Local<Stride>,
    mut kick: ResMut<GunKick>,
    mut react: ResMut<Reactions>,
    mut noise: MessageWriter<Noise>,
    rigs: Query<&Rig>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    // Idle, into the aim, into the menu (never both).
    let aim = player.aim;
    let mut p = IDLE.lerp(&AIM, aim).lerp(&MENU, player.menu * (1.0 - aim));

    // He looks, and aims, up and down with the camera.
    let pitch = player.look.y;
    p.head.x += pitch * 0.5;
    p.spine.x += pitch * 0.25 * aim;
    p.chest.x += pitch * 0.25 * aim;
    p.gun_rot.x += pitch * aim;
    p.gun.y += pitch * 0.3 * aim;

    // Walking: each foot swings ahead along the way he goes while the
    // other carries him, the hips sink into it, the chest counters.
    let speed = player.velocity.length();
    let going = Quat::from_rotation_y(-player.yaw) * player.velocity.normalize_or_zero();
    let hurry = ((speed - WALK) / (RUN - WALK)).clamp(0.0, 1.0);
    let walking = if speed > 0.2 { 1.0 } else { 0.0 };
    stride.amount += (walking - stride.amount) * (1.0 - (-8.0 * dt).exp());
    let before = stride.phase;
    stride.phase += std::f32::consts::TAU * (1.5 + 0.7 * hurry) * dt * stride.amount;
    let (phase, amount) = (stride.phase, stride.amount);
    for (i, foot) in p.feet.iter_mut().enumerate() {
        let swing = phase + i as f32 * std::f32::consts::PI;
        *foot += going * (0.3 + 0.1 * hurry) * swing.cos() * amount;
        foot.y += (0.09 + 0.04 * hurry) * (-swing.sin()).max(0.0) * amount;
    }
    p.hips.y -= (0.05 + 0.04 * hurry - 0.012 * (2.0 * phase).cos()) * amount;
    p.chest.y += 0.07 * phase.sin() * amount;
    p.spine.x -= 0.07 * hurry * amount;
    // A foot lands every half turn.
    let step = std::f32::consts::PI;
    if (before / step).floor() != (phase / step).floor() {
        noise.write(Noise("step", player.at));
    }

    // Breathing and a slow shift of weight, less of both while aiming.
    let calm = 1.0 - 0.6 * aim;
    p.chest.x += (t * 1.7).sin() * 0.018 * calm;
    p.hips.x += (t * 0.42).sin() * 0.014 * calm;
    p.hips_rot.z += (t * 0.42).sin() * 0.02 * calm;
    p.head.y += (t * 0.31).sin() * 0.06 * calm;

    // Reactions.
    react.flinch = (react.flinch - dt * 2.8).max(0.0);
    let f = react.flinch * react.flinch;
    p.spine.x += 0.22 * f;
    p.chest.x += 0.3 * f;
    p.chest.z += react.flinch_roll * f;
    p.head.x += 0.25 * f;
    kick.recoil = (kick.recoil - dt * 7.0).max(0.0);
    p.gun += Vec3::new(0.0, 0.025, 0.07) * kick.recoil;
    p.gun_rot.x += 0.16 * kick.recoil;
    let mut fore = FOREGRIP;
    if let Some(r) = kick.reload {
        let r = (r + dt / 0.9).min(1.0);
        kick.reload = (r < 1.0).then_some(r);
        let k = (r * std::f32::consts::PI).sin();
        p.gun_rot.z -= 0.7 * k;
        p.gun_rot.x -= 0.25 * k;
        p.gun.y -= 0.07 * k;
        p.grip = p.grip.max(k);
        fore = FOREGRIP.lerp(Vec3::new(0.0, -0.16, 0.0), k);
    }
    let throw = react.throw.map(|s| {
        let s = s + dt / 0.6;
        react.throw = (s < 1.0).then_some(s);
        (s.min(1.0) * std::f32::consts::PI).sin()
    });

    for rig in &rigs {
        // The trunk, in the root's frame.
        let hips = Transform::from_translation(HIPS + p.hips).with_rotation(euler(p.hips_rot));
        let spine_local = Transform::from_translation(SPINE).with_rotation(euler(p.spine));
        let chest_local = Transform::from_translation(CHEST).with_rotation(euler(p.chest));
        let chest = hips * spine_local * chest_local;
        let gun = Transform::from_translation(p.gun).with_rotation(euler(p.gun_rot));
        set(&mut transforms, rig.hips, hips);
        set(&mut transforms, rig.spine, spine_local);
        set(&mut transforms, rig.chest, chest_local);
        set(
            &mut transforms,
            rig.head,
            Transform::from_translation(NECK).with_rotation(euler(p.head)),
        );
        set(&mut transforms, rig.gun, gun);

        // Arms: the right hand on the grip; the left on the foregrip, or
        // hanging, or thrown forward for stasis.
        for (i, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let shoulder = chest.transform_point(SHOULDER * Vec3::new(side, 1.0, 1.0));
            // The free arm swings against its leg.
            let swing = going * 0.12 * (phase + i as f32 * std::f32::consts::PI).cos() * amount;
            let hang = shoulder + Vec3::new(side * 0.06, -0.5, 0.06) - swing;
            let mut hand = if i == 0 {
                gun.transform_point(GRIP)
            } else {
                hang.lerp(gun.transform_point(fore), p.grip)
            };
            if i == 1
                && let Some(k) = throw
            {
                hand = hand.lerp(shoulder + Vec3::new(0.12, 0.02, -0.5), k);
            }
            let pole = shoulder + Vec3::new(side * 0.35, -0.3, 0.45);
            let (elbow, wrist) = two_bone(shoulder, hand, UPPER_ARM, FOREARM, pole - shoulder);
            let upper = limb(shoulder, elbow, pole - shoulder);
            let fore_rot = limb(elbow, wrist, pole - elbow);
            let [s, e, w] = rig.arms[i];
            set_rot(&mut transforms, s, chest.rotation.inverse() * upper);
            set_rot(&mut transforms, e, upper.inverse() * fore_rot);
            set_rot(&mut transforms, w, Quat::IDENTITY);
        }

        // Legs: feet planted, knees forward.
        for (i, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let hip = hips.transform_point(HIP * Vec3::new(side, 1.0, 1.0));
            let front = Vec3::new(side * 0.15, 0.0, -1.0);
            let (knee, ankle) = two_bone(hip, p.feet[i], THIGH, SHIN, front);
            let thigh = limb(hip, knee, -front);
            let shin = limb(knee, ankle, -front);
            let [h, k, a] = rig.legs[i];
            set_rot(&mut transforms, h, hips.rotation.inverse() * thigh);
            set_rot(&mut transforms, k, thigh.inverse() * shin);
            // The boot flat on the floor, turned with the stance.
            let foot = Quat::from_rotation_y(p.hips_rot.y * 0.6);
            set_rot(&mut transforms, a, shin.inverse() * foot);
        }
    }
}

fn set(transforms: &mut Query<&mut Transform>, e: Entity, to: Transform) {
    if let Ok(mut t) = transforms.get_mut(e) {
        *t = to;
    }
}

fn set_rot(transforms: &mut Query<&mut Transform>, e: Entity, to: Quat) {
    if let Ok(mut t) = transforms.get_mut(e) {
        t.rotation = to;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The solved chain keeps its bone lengths and ends on the target when
    /// it can reach it.
    #[test]
    fn two_bone_reaches_and_keeps_lengths() {
        let s = Vec3::new(0.2, 1.5, 0.0);
        let t = Vec3::new(0.1, 1.2, -0.35);
        let (mid, end) = two_bone(s, t, UPPER_ARM, FOREARM, Vec3::new(0.3, -0.3, 0.4));
        assert!((mid.distance(s) - UPPER_ARM).abs() < 1e-4);
        assert!((end.distance(mid) - FOREARM).abs() < 1e-4);
        assert!(end.distance(t) < 1e-4);
        // The limb rotation carries rest −Y onto the bone.
        let q = limb(s, mid, Vec3::Z);
        assert!((q * Vec3::NEG_Y).distance((mid - s).normalize()) < 1e-4);
    }
}
