//! The user-space cursor of JSX `<svg>` shape children.
//!
//! Shapes are **Node-less** entities, hit through
//! [`super::pick::refine_svg_pointer_hits`], which puts the topmost painted
//! shape under a pointer in the picking hover map. The core derives hover,
//! press and drags from that map for every element, so a shape needs no
//! interaction system of its own — only its position: a shape has no box,
//! and its events report `x`/`y` in SVG **user units**.
//! [`sync_shape_event_pos`] publishes the refined hit's user-space cursor in
//! [`EventLocalPos`], which the core's hover/pointer collectors report
//! instead of a node-relative position.
//!
//! Only handler-bearing shapes carry the slot (the core's pointer stamps give
//! it to a node-less element exactly when it declares `onClick`/
//! `onPointer*`); a handler-less shape is skipped, and its hits fall through
//! to the `<svg>` root's own handlers via the click collector's `ChildOf`
//! climb.

use bevy::picking::hover::HoverMap;
use bevy::prelude::*;

use super::SvgShape;
use super::pick::SvgPointerShapeHits;
// The user-space cursor slot: the contract component the event collectors
// read (`x`/`y` in SVG user units while hovered, `None` otherwise). Stamped/
// removed alongside the shape's handler components by the core's node-less
// pointer stamps.
use bevy_react_core::ext::EventLocalPos;

/// Fill [`EventLocalPos`] on the shape each pointer hovers, from this frame's
/// refined shape hits, and clear it on the shapes left. Runs in
/// `InteractionSyncSet`, before the core's hover/pointer collectors.
///
/// Costs the pointers' hits, not the shape count; every write is
/// compare-before-write. When two pointers hover one shape the last hit
/// wins — acceptable: a virtual pointer only exists where the real one is
/// suppressed.
pub(crate) fn sync_shape_event_pos(
    hover_map: Option<Res<HoverMap>>,
    shape_hits: Res<SvgPointerShapeHits>,
    mut shapes: Query<&mut EventLocalPos, With<SvgShape>>,
    // The shapes the last run filled.
    mut live: Local<Vec<Entity>>,
) {
    let mut filled = Vec::new();
    for (pointer, hit) in &shape_hits.hits {
        // A refined hit only counts while its shape is hovered: something
        // above the `<svg>` may keep it out of the hover map.
        let hovered = hover_map
            .as_ref()
            .and_then(|map| map.get(pointer))
            .is_some_and(|hits| hits.contains_key(&hit.shape));
        if hovered && let Ok(mut slot) = shapes.get_mut(hit.shape) {
            slot.set_if_neq(EventLocalPos(Some(hit.user_pos)));
            filled.push(hit.shape);
        }
    }
    for shape in live.drain(..) {
        if !filled.contains(&shape)
            && let Ok(mut slot) = shapes.get_mut(shape)
        {
            slot.set_if_neq(EventLocalPos(None));
        }
    }
    *live = filled;
}

#[cfg(test)]
mod tests {
    use bevy::ecs::entity::EntityHashMap;
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::PointerId;

    use super::super::pick::SvgShapeHit;
    use super::*;
    use crate::{ShapeAttrs, ShapeKind};

    /// An app running the sync, with one `<svg>` root entity and one
    /// handler-bearing shape (it carries the slot). Returns
    /// `(app, root, shape)`.
    fn shape_app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<SvgPointerShapeHits>()
            .add_systems(Update, sync_shape_event_pos);
        let root = app.world_mut().spawn_empty().id();
        let shape = app
            .world_mut()
            .spawn((bare_shape(), ChildOf(root), EventLocalPos::default()))
            .id();
        (app, root, shape)
    }

    fn bare_shape() -> SvgShape {
        SvgShape {
            kind: ShapeKind::Rect,
            attrs: ShapeAttrs::default(),
        }
    }

    /// Put `entity` (alone) under `pointer` in the hover map.
    fn hover(world: &mut World, pointer: PointerId, entity: Entity) {
        let mut map = HoverMap::default();
        let mut entry: EntityHashMap<HitData> = EntityHashMap::default();
        entry.insert(entity, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
        map.insert(pointer, entry);
        world.insert_resource(map);
    }

    /// Record the refined hit for `pointer`.
    fn set_hit(world: &mut World, pointer: PointerId, shape: Entity, user_pos: Vec2) {
        world.resource_mut::<SvgPointerShapeHits>().hits.insert(
            pointer,
            SvgShapeHit {
                shape,
                user_pos,
                depth: 0.0,
            },
        );
    }

    fn slot(app: &App, shape: Entity) -> Option<Vec2> {
        app.world().get::<EventLocalPos>(shape).unwrap().0
    }

    /// A hovered shape publishes the refined hit's user-space cursor; once
    /// the pointer is off it, the slot is cleared — nothing sticky.
    #[test]
    fn hovered_shape_publishes_its_user_space_cursor() {
        let (mut app, _root, shape) = shape_app();
        let at = Vec2::new(25.0, 50.0);
        hover(app.world_mut(), PointerId::Mouse, shape);
        set_hit(app.world_mut(), PointerId::Mouse, shape, at);
        app.update();
        assert_eq!(slot(&app, shape), Some(at));

        // The pointer moves off: empty hover map, hits rebuilt empty.
        app.world_mut().insert_resource(HoverMap::default());
        app.world_mut()
            .resource_mut::<SvgPointerShapeHits>()
            .hits
            .clear();
        app.update();
        assert_eq!(slot(&app, shape), None);
    }

    /// A refined hit whose shape is not hovered — something above the
    /// `<svg>` blocks the pointer — publishes nothing.
    #[test]
    fn a_blocked_hit_publishes_nothing() {
        let (mut app, _root, shape) = shape_app();
        set_hit(app.world_mut(), PointerId::Mouse, shape, Vec2::ONE);
        app.update();
        assert_eq!(slot(&app, shape), None);
    }

    /// A handler-less shape carries no slot and must not grow one, hovered
    /// or not.
    #[test]
    fn a_handlerless_shape_is_left_bare() {
        let (mut app, root, _shape) = shape_app();
        let bare = app.world_mut().spawn((bare_shape(), ChildOf(root))).id();
        hover(app.world_mut(), PointerId::Mouse, bare);
        set_hit(app.world_mut(), PointerId::Mouse, bare, Vec2::ONE);
        app.update();
        assert!(app.world().get::<EventLocalPos>(bare).is_none());
    }
}
