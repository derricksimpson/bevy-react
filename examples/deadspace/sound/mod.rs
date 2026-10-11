//! The sound, synthesized in code at startup — no audio files: every cue
//! React plays by name (`sound.cue`, see `ui/src/sound.ts`), the station's
//! drone, and the suit's breathing that takes over when the room loses its
//! air (`suit.vacuum`: the drone sinks, the breath rises — sound doesn't
//! carry in a vacuum).
//!
//! The room's own noises (`station::Noise`: the doors, the elevator, a bolt
//! striking the wall) play too, quieter the farther they are from the
//! camera.
//!
//! Every sound renders (`sfx.rs`, `gear.rs`, `world.rs`, `ambience.rs`, on
//! the `kit.rs` kit) into
//! a buffer of samples held by a [`Synth`] asset, which bevy_audio plays like
//! a decoded file (a custom `Decodable`, so no codec feature is needed).

mod ambience;
mod gear;
mod kit;
mod sfx;
mod world;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source, Volume};
use bevy::prelude::*;
use bevy_react::{ReactAppExt, react_message};

use crate::engineer::SuitVacuum;
use crate::station::Noise;
use crate::tour::MainCamera;
use crate::weapons::Equip;
use kit::RATE;
use sfx::SFX;

/// A cue restarted within this many seconds of itself is dropped: a quick
/// sweep over a list would otherwise stack its ticks into a buzz.
const RETRIGGER: f64 = 0.04;
/// Seconds the drone takes to fade in at startup.
const FADE_IN: f32 = 4.0;
/// The drone's level, and how far it sinks in a vacuum.
const DRONE: f32 = 0.9;
const MUFFLED: f32 = 0.25;
/// The levels move toward their targets this fast (per second).
const MIX_RATE: f32 = 1.2;

/// React → Bevy: play a named cue.
#[react_message(name = "sound.cue")]
pub struct Cue {
    pub name: String,
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_audio_source::<Synth>()
            .init_resource::<Mix>()
            .add_systems(Startup, synthesize)
            .add_systems(Update, (mix_loops, world_noises));
    }
}

/// The sound's bindings (shared with the `--export-bindings` path). It
/// also listens to the suit and the weapons (their modules register those
/// messages).
pub fn register_bindings(app: &mut App) {
    app.add_react_handler(play)
        .add_react_handler(on_vacuum)
        .add_react_handler(on_equip);
}

/// The cues: name, sound, when it last started (the retrigger guard).
#[derive(Resource)]
struct Bank(Vec<(&'static str, Handle<Synth>, f64)>);

/// What the loops should do: the vacuum, and the weapon whose shot `fire`
/// plays.
#[derive(Resource)]
struct Mix {
    vacuum: bool,
    weapon: String,
}

impl Default for Mix {
    fn default() -> Self {
        Self {
            vacuum: false,
            weapon: "cutter".into(),
        }
    }
}

/// The two loops' players.
#[derive(Component)]
enum Loop {
    Drone,
    Breath,
}

/// Render every sound and start the loops (silent; `mix_loops` brings them
/// in).
fn synthesize(mut commands: Commands, mut synths: ResMut<Assets<Synth>>) {
    let bank = SFX
        .iter()
        .chain(&world::NOISES)
        .map(|&(name, render)| (name, synths.add(Synth::new(1, render())), f64::NEG_INFINITY))
        .collect();
    commands.insert_resource(Bank(bank));
    for (samples, channels, which) in [
        (ambience::drone(), 2, Loop::Drone),
        (ambience::breath(), 1, Loop::Breath),
    ] {
        commands.spawn((
            AudioPlayer(synths.add(Synth::new(channels, samples))),
            PlaybackSettings::LOOP.with_volume(Volume::SILENT),
            which,
        ));
    }
}

fn play(
    cue: On<Cue>,
    mut bank: ResMut<Bank>,
    mix: Res<Mix>,
    time: Res<Time<Real>>,
    mut unknown: Local<HashSet<String>>,
    mut commands: Commands,
) {
    let name = if cue.name == "fire" {
        format!("fire-{}", mix.weapon)
    } else {
        cue.name.clone()
    };
    // In a vacuum only what the suit carries is heard, and that muffled.
    let level = if mix.vacuum { 0.55 } else { 1.0 };
    let now = time.elapsed_secs_f64();
    if !bank.play(&name, level, now, &mut commands) && unknown.insert(name.clone()) {
        warn!("sound.cue: no sound named {name:?}");
    }
}

/// Play the room's noises, fading with distance from the camera (and all
/// but gone in a vacuum).
fn world_noises(
    mut noises: MessageReader<Noise>,
    mut bank: ResMut<Bank>,
    mix: Res<Mix>,
    time: Res<Time<Real>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut unknown: Local<HashSet<&'static str>>,
    mut commands: Commands,
) {
    let ear = camera
        .single()
        .map_or(Vec3::ZERO, GlobalTransform::translation);
    let now = time.elapsed_secs_f64();
    for Noise(name, at) in noises.read() {
        let name = if *name == "elevatorArrive" {
            "elevator"
        } else {
            name
        };
        let near = 1.0 / (1.0 + 0.12 * ear.distance(*at));
        let level = near * if mix.vacuum { 0.15 } else { 1.0 };
        if !bank.play(name, level, now, &mut commands) && unknown.insert(name) {
            warn!("station::Noise: no sound named {name:?}");
        }
    }
}

impl Bank {
    /// Start sound `name` at `level` unless it started within
    /// [`RETRIGGER`]; whether `name` exists.
    fn play(&mut self, name: &str, level: f32, now: f64, commands: &mut Commands) -> bool {
        let Some((_, sound, last)) = self.0.iter_mut().find(|(n, ..)| *n == name) else {
            return false;
        };
        if now - *last >= RETRIGGER {
            *last = now;
            // ponytail: without an audio device bevy never plays (nor
            // despawns) these; a session's worth of inert entities, no
            // per-frame cost.
            commands.spawn((
                AudioPlayer(sound.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(level)),
            ));
        }
        true
    }
}

fn on_vacuum(vacuum: On<SuitVacuum>, mut mix: ResMut<Mix>) {
    mix.vacuum = vacuum.on;
}

fn on_equip(equip: On<Equip>, mut mix: ResMut<Mix>) {
    mix.weapon = equip.weapon.clone();
}

/// Ease each loop toward its level: the drone faded in at startup and sunk
/// in a vacuum, the breath only in one. Compare-before-write.
fn mix_loops(time: Res<Time<Real>>, mix: Res<Mix>, mut loops: Query<(&Loop, &mut AudioSink)>) {
    let fade_in = (time.elapsed_secs() / FADE_IN).min(1.0);
    let step = MIX_RATE * time.delta_secs();
    for (which, mut sink) in &mut loops {
        let target = match which {
            Loop::Drone => DRONE * fade_in * if mix.vacuum { MUFFLED } else { 1.0 },
            Loop::Breath => {
                if mix.vacuum {
                    0.8
                } else {
                    0.0
                }
            }
        };
        let now = sink.volume().to_linear();
        if now != target {
            let next = now + (target - now).clamp(-step, step);
            sink.set_volume(Volume::Linear(next));
        }
    }
}

/// A synthesized sound: interleaved samples at [`RATE`], played from memory.
#[derive(Asset, TypePath, Clone)]
struct Synth {
    channels: ChannelCount,
    samples: Arc<[f32]>,
}

impl Synth {
    fn new(channels: u16, samples: Vec<f32>) -> Self {
        Self {
            channels: ChannelCount::new(channels).unwrap(),
            samples: samples.into(),
        }
    }
}

impl Decodable for Synth {
    type Decoder = Playhead;

    fn decoder(&self) -> Playhead {
        Playhead(self.clone(), 0)
    }
}

/// A [`Synth`] playing: the sound and its next sample.
struct Playhead(Synth, usize);

impl Iterator for Playhead {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.0.samples.get(self.1).copied();
        self.1 += 1;
        sample
    }
}

impl Source for Playhead {
    /// One span: the whole buffer, then `Some(0)` once played out.
    fn current_span_len(&self) -> Option<usize> {
        let len = self.0.samples.len();
        Some(if self.1 < len { len } else { 0 })
    }

    fn channels(&self) -> ChannelCount {
        self.0.channels
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
