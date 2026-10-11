//! Fabricated pointer state for the collectors' tests: pointer entities, a
//! hover map and a press state written by hand.

use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::system::RunSystemOnce;
use bevy::picking::backend::HitData;
use bevy::picking::events::PointerState;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{
    Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation, PointerMap,
};
use bevy::platform::time::Instant;
use bevy::prelude::*;

/// A location on an image target (no camera needs to match it).
pub(crate) fn at(position: Vec2) -> Location {
    Location {
        target: NormalizedRenderTarget::Image(Handle::<Image>::default().into()),
        position,
    }
}

pub(crate) fn hit(depth: f32) -> HitData {
    HitData::new(Entity::PLACEHOLDER, depth, None, None)
}

/// Spawn the pointer entity for `id` at `position`.
pub(crate) fn spawn_pointer(world: &mut World, id: PointerId, position: Vec2) -> Entity {
    world.init_resource::<PointerMap>();
    world.spawn((id, PointerLocation::new(at(position)))).id()
}

/// Move the pointer entity `pointer` to `position`.
pub(crate) fn move_to(world: &mut World, pointer: Entity, position: Vec2) {
    world.get_mut::<PointerLocation>(pointer).unwrap().location = Some(at(position));
}

/// Replace `pointer`'s hover-map entry with `entities` (topmost first).
pub(crate) fn hover(world: &mut World, pointer: PointerId, entities: &[Entity]) {
    let entry: EntityHashMap<HitData> = entities
        .iter()
        .enumerate()
        .map(|(i, &entity)| (entity, hit(i as f32 * 0.00001)))
        .collect();
    world
        .get_resource_or_init::<HoverMap>()
        .insert(pointer, entry);
}

/// Hold `pointer`'s primary button down, as `PointerInput::receive`
/// records it on the pointer entity.
pub(crate) fn hold(world: &mut World, pointer: PointerId) {
    world.init_resource::<Messages<PointerInput>>();
    world.write_message(PointerInput::new(
        pointer,
        at(Vec2::ZERO),
        PointerAction::Press(PointerButton::Primary),
    ));
    world.run_system_once(PointerInput::receive).unwrap();
}

/// Replace the set `pointer`'s `button` is pressing.
pub(crate) fn press(
    world: &mut World,
    pointer: PointerId,
    button: PointerButton,
    entities: &[Entity],
) {
    let mut state = world.get_resource_or_init::<PointerState>();
    let pressing = &mut state.get_mut(pointer, button).pressing;
    pressing.clear();
    for &entity in entities {
        pressing.insert(entity, (at(Vec2::ZERO), Instant::now(), hit(0.0)));
    }
}
