//! `em` on an element with no font of its own.

use bevy::prelude::*;
use bevy::text::DEFAULT_REM_SIZE_PX;

use crate::bridge::ReactNode;

/// `em` on an element with no font of its own is the root size (CSS: the
/// inherited font size, and nothing between the root and a `<node>` sets
/// one). Bevy derives `EmSize` only where a `TextFont` sits and leaves every
/// other node at its 20px default whatever `RemSize` says, so mirror
/// `RemSize` onto the other React nodes: all of them when it changes, the
/// freshly mounted ones while it is not the default. `Without<TextFont>`
/// keeps this disjoint from bevy's `sync_font_size_to_em_size`.
///
/// Nothing runs at the default root size. At another one the `Added` filter
/// costs a tick compare per fontless React node per frame.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_em_size_to_rem(
    rem: Res<RemSize>,
    mut nodes: ParamSet<(
        Query<&mut EmSize, (With<ReactNode>, Without<TextFont>)>,
        Query<&mut EmSize, (Added<ReactNode>, Without<TextFont>)>,
    )>,
) {
    let size = EmSize(rem.0);
    if rem.is_changed() {
        for mut em in &mut nodes.p0() {
            em.set_if_neq(size);
        }
    } else if rem.0 != DEFAULT_REM_SIZE_PX {
        // (At the default a fresh node's own `EmSize` is already right.)
        for mut em in &mut nodes.p1() {
            em.set_if_neq(size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transition::layout_tests::layout_app;

    /// Real layout plus the stamp, and a full-viewport root to lay out in.
    fn em_app() -> (App, Entity) {
        let mut app = layout_app();
        app.add_systems(
            PostUpdate,
            sync_em_size_to_rem.before(bevy::ui::ui_layout_system),
        );
        let root = app
            .world_mut()
            .spawn(Node {
                width: Val::Vw(100.0),
                height: Val::Vh(100.0),
                align_items: AlignItems::FlexStart,
                ..default()
            })
            .id();
        (app, root)
    }

    fn sized(width: Val, height: Val) -> Node {
        Node {
            width,
            height,
            ..default()
        }
    }

    fn size_of(app: &App, e: Entity) -> Vec2 {
        app.world().entity(e).get::<ComputedNode>().unwrap().size
    }

    /// On a fontless React node `em` is the root size, like `rem` — and both
    /// follow a `RemSize` change (bevy alone would leave the `em` at 20px).
    #[test]
    fn rem_and_em_follow_the_root_size() {
        let (mut app, root) = em_app();
        let e = app
            .world_mut()
            .spawn((
                ReactNode(1),
                sized(Val::Rem(2.0), Val::Em(1.0)),
                ChildOf(root),
            ))
            .id();
        app.update();
        assert_eq!(size_of(&app, e), Vec2::new(40.0, 20.0));

        app.insert_resource(RemSize(10.0));
        app.update();
        assert_eq!(size_of(&app, e), Vec2::new(20.0, 10.0));

        // Back at the default: every node is re-stamped, not just skipped.
        app.insert_resource(RemSize(DEFAULT_REM_SIZE_PX));
        app.update();
        assert_eq!(size_of(&app, e), Vec2::new(40.0, 20.0));
    }

    /// A node mounted while the root size is not the default starts at it —
    /// with no write to `RemSize` that frame.
    #[test]
    fn fresh_node_under_a_non_default_root_size() {
        let (mut app, root) = em_app();
        app.insert_resource(RemSize(10.0));
        app.update();
        let e = app
            .world_mut()
            .spawn((
                ReactNode(1),
                sized(Val::Em(2.0), Val::Px(5.0)),
                ChildOf(root),
            ))
            .id();
        app.update();
        assert_eq!(size_of(&app, e).x, 20.0);
    }

    /// A text box's `em` is its own font size: bevy's sync owns that
    /// `EmSize`, and the stamp leaves it alone.
    #[test]
    fn a_text_box_em_is_its_own_font_size() {
        let (mut app, root) = em_app();
        let e = app
            .world_mut()
            .spawn((
                ReactNode(1),
                sized(Val::Em(2.0), Val::Px(5.0)),
                TextFont {
                    font_size: FontSize::Px(30.0),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        app.update();
        assert_eq!(size_of(&app, e).x, 60.0);
        app.insert_resource(RemSize(10.0));
        app.update();
        assert_eq!(size_of(&app, e).x, 60.0);
    }

    /// An app's own UI node (no `ReactNode`) keeps the `EmSize` it was given.
    #[test]
    fn foreign_nodes_keep_their_em_size() {
        let (mut app, root) = em_app();
        let e = app
            .world_mut()
            .spawn((Node::default(), EmSize(7.0), ChildOf(root)))
            .id();
        app.update();
        app.insert_resource(RemSize(10.0));
        app.update();
        assert_eq!(app.world().entity(e).get::<EmSize>(), Some(&EmSize(7.0)));
    }
}
