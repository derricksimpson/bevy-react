//! [`AnimationRuns`]: how the devtools Animations tab finds the animations
//! a component runs — the core's transition state and a feature's own (the
//! svg shapes' attribute transitions) alike.

use std::any::TypeId;

use bevy::ecs::component::Mutable;
use bevy::prelude::*;

use crate::animations::Run;

/// A component whose in-flight [`Run`]s the devtools Animations tab lists
/// under the entity's node and can scrub. Implement it on a feature's
/// animation state and register the type with
/// [`ReactAppExt::add_react_animation_runs`](crate::ReactAppExt::add_react_animation_runs);
/// the core registers its transition state the same way.
pub trait AnimationRuns: Component<Mutability = Mutable> {
    /// Visit every run in flight with the label its line shows: what the
    /// app wrote to start it (a `transition` key, an attribute name). Runs
    /// sharing a label are one line, scrubbed together; the first one
    /// visited gives the line its timeline.
    fn visit_runs(&mut self, f: &mut dyn FnMut(&'static str, &mut Run));
}

/// The registered [`AnimationRuns`] types, as monomorphized visitors over
/// the world (read by the devtools, which own the world while they sample
/// and seek).
#[derive(Resource, Default)]
pub(crate) struct AnimationRunSources(Vec<(TypeId, RunSource)>);

type VisitAll = fn(&mut World, &mut dyn FnMut(Entity, &'static str, &mut Run));
type VisitOne = fn(&mut World, Entity, &mut dyn FnMut(&'static str, &mut Run));

/// One registered type's visitors.
#[derive(Clone, Copy)]
#[cfg_attr(not(feature = "devtools"), allow(dead_code))]
pub(crate) struct RunSource {
    /// Every entity's runs — read only: it bypasses change detection.
    pub all: VisitAll,
    /// One entity's runs, to move them (marks the component changed).
    pub one: VisitOne,
}

impl AnimationRunSources {
    pub(crate) fn add<C: AnimationRuns>(&mut self) {
        if self.0.iter().all(|(id, _)| *id != TypeId::of::<C>()) {
            self.0.push((
                TypeId::of::<C>(),
                RunSource {
                    all: visit_all::<C>,
                    one: visit_one::<C>,
                },
            ));
        }
    }

    #[cfg_attr(not(feature = "devtools"), allow(dead_code))]
    pub(crate) fn sources(&self) -> Vec<RunSource> {
        self.0.iter().map(|(_, s)| *s).collect()
    }
}

fn visit_all<C: AnimationRuns>(
    world: &mut World,
    f: &mut dyn FnMut(Entity, &'static str, &mut Run),
) {
    let mut query = world.query::<(Entity, &mut C)>();
    for (entity, mut c) in query.iter_mut(world) {
        c.bypass_change_detection()
            .visit_runs(&mut |label, run| f(entity, label, run));
    }
}

fn visit_one<C: AnimationRuns>(
    world: &mut World,
    entity: Entity,
    f: &mut dyn FnMut(&'static str, &mut Run),
) {
    if let Some(mut c) = world.get_mut::<C>(entity) {
        c.visit_runs(f);
    }
}
