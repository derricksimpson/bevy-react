//! The `cursor` style prop: drives the window's mouse cursor from the node the
//! pointer is over.
//!
//! `bevy_ui` has no per-node cursor mechanism — `bevy_winit` only reads
//! [`CursorIcon`] from the window entity. So a node's `cursor` style is stamped
//! as a [`NodeCursor`] component (in [`crate::ui_map::apply_style_masked`]) and
//! [`drive_cursor_icon`] each frame picks the node under the pointer and writes the
//! chosen cursor onto the primary window's [`CursorIcon`].
//!
//! The node under the pointer is read from the picking [`HoverMap`] — the
//! same authoritative hover state hover styling and clicks ride — so the
//! cursor follows what is actually hit: a node clipped away by a scroll
//! container, hidden, or under a `focusPolicy: "block"` node sets nothing.
//! Among the hovered nodes the topmost one that has a cursor, its own or its
//! nearest cursor-bearing ancestor's (CSS-like inheritance), decides; nodes
//! without one are transparent, so a pass-through overlay shows the cursor of
//! what is beneath it.
//!
//! Three pointers feed the one window cursor, in this order, so they never
//! clobber each other:
//! - **Virtual pointers** (a `<surface>`'s offscreen subtree, hit-tested in
//!   the *texture's* space): each registered
//!   [`VirtualPointers`](crate::ext::VirtualPointers) entry.
//! - **The transform3d pointer**: the window cursor remapped into a
//!   3D-transformed layer, where the visual is (the mouse's own hits on those
//!   members are suppressed — their layout rects are stale).
//! - **The mouse**, over everything else in the window.
//!
//! With no cursor-bearing node under any of them: the default arrow.

use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
#[cfg(feature = "custom_cursor")]
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};
#[cfg(feature = "custom_cursor")]
use bevy::window::{CustomCursor, CustomCursorImage};

/// The cursor a node requests while the pointer is over it — the raw `cursor` style
/// name (a system keyword or a custom-cursor name), resolved at drive time by
/// [`resolve_cursor`]. Read by [`drive_cursor_icon`]; absent → the node contributes
/// no cursor.
#[derive(Component, Debug, Clone)]
pub struct NodeCursor(pub String);

/// Named custom image cursors, registered upfront via `ReactUiPlugin::cursor` and
/// loaded to handles in the plugin's `Startup` `setup` (mirroring the
/// [`Fonts`](crate::Fonts) registry). Keyed by the name React selects with
/// `style={{ cursor: name }}`; the app owns the asset, React references it by name.
#[cfg(feature = "custom_cursor")]
#[derive(Resource, Default)]
pub struct CustomCursors(pub HashMap<String, CustomCursorImage>);

/// Without the `custom_cursor` feature there are no custom cursors (bevy's
/// `CustomCursor` is compiled out and `ReactUiPlugin::cursor` with it): an
/// empty stand-in keeps the drive's one signature.
#[cfg(not(feature = "custom_cursor"))]
#[derive(Resource, Default)]
pub struct CustomCursors;

/// Resolve a `cursor` name to the [`CursorIcon`] to write on the window. The
/// [`CustomCursors`] registry is checked **first**, so a custom cursor registered under
/// a system-keyword name (e.g. `"pointer"`) *overrides* that built-in; otherwise the
/// name is matched against the system keywords, and finally an unknown name warns and
/// falls back to the default arrow (mirroring an unknown `fontFamily`).
#[cfg_attr(not(feature = "custom_cursor"), allow(unused_variables))]
fn resolve_cursor(name: &str, custom: &CustomCursors) -> CursorIcon {
    #[cfg(feature = "custom_cursor")]
    if let Some(image) = custom.0.get(name) {
        return CursorIcon::Custom(CustomCursor::Image(image.clone()));
    }
    if let Some(icon) = system_cursor_keyword(name) {
        CursorIcon::from(icon)
    } else {
        #[cfg(feature = "custom_cursor")]
        let msg = format!("unknown cursor {name:?}");
        #[cfg(not(feature = "custom_cursor"))]
        let msg = format!(
            "unknown cursor {name:?} (custom cursors need bevy-react's `custom_cursor` feature)"
        );
        crate::diag::report("cursor", name, &msg);
        CursorIcon::from(SystemCursorIcon::Default)
    }
}

/// Map a CSS `cursor` keyword (camelCase or CSS kebab-case) to a built-in
/// [`SystemCursorIcon`], or `None` if the string is not a reserved keyword (→ an
/// unregistered custom name). The full winit set.
fn system_cursor_keyword(s: &str) -> Option<SystemCursorIcon> {
    use SystemCursorIcon::*;
    Some(match s {
        "default" | "auto" => Default,
        "contextMenu" | "context-menu" => ContextMenu,
        "help" => Help,
        "pointer" => Pointer,
        "progress" => Progress,
        "wait" => Wait,
        "cell" => Cell,
        "crosshair" => Crosshair,
        "text" => Text,
        "verticalText" | "vertical-text" => VerticalText,
        "alias" => Alias,
        "copy" => Copy,
        "move" => Move,
        "noDrop" | "no-drop" => NoDrop,
        "notAllowed" | "not-allowed" => NotAllowed,
        "grab" => Grab,
        "grabbing" => Grabbing,
        "eResize" | "e-resize" => EResize,
        "nResize" | "n-resize" => NResize,
        "neResize" | "ne-resize" => NeResize,
        "nwResize" | "nw-resize" => NwResize,
        "sResize" | "s-resize" => SResize,
        "seResize" | "se-resize" => SeResize,
        "swResize" | "sw-resize" => SwResize,
        "wResize" | "w-resize" => WResize,
        "ewResize" | "ew-resize" => EwResize,
        "nsResize" | "ns-resize" => NsResize,
        "neswResize" | "nesw-resize" => NeswResize,
        "nwseResize" | "nwse-resize" => NwseResize,
        "colResize" | "col-resize" => ColResize,
        "rowResize" | "row-resize" => RowResize,
        "allScroll" | "all-scroll" => AllScroll,
        "zoomIn" | "zoom-in" => ZoomIn,
        "zoomOut" | "zoom-out" => ZoomOut,
        _ => return None,
    })
}

/// Set the primary window's [`CursorIcon`] to the cursor of the node under the
/// pointer (main-window or `<surface>` UI). When the pointer is over no cursor-bearing
/// node, it falls back to the `"default"` cursor — which resolves like any other name,
/// so a custom cursor registered under `"default"` becomes the app-wide base cursor
/// (and with none registered, the system default arrow).
///
/// Writes only on change, so a steady hover doesn't re-trigger `bevy_winit`'s
/// `update_cursors` every frame. Runs after `apply_js_ops` so a freshly-stamped
/// `NodeCursor` is visible; the hover map is this frame's (`PreUpdate`).
#[allow(clippy::too_many_arguments)]
pub fn drive_cursor_icon(
    mut commands: Commands,
    windows: Query<(Entity, Option<&CursorIcon>), With<PrimaryWindow>>,
    // `Option`: an app without picking has no hover, so the default cursor.
    hover_map: Option<Res<HoverMap>>,
    virtual_pointers: Option<Res<crate::ext::VirtualPointers>>,
    transform3d_pointer: Option<Res<crate::layer::pick3d::Transform3dPointer>>,
    custom_cursors: Res<CustomCursors>,
    node_cursors: Query<&NodeCursor>,
    child_of: Query<&ChildOf>,
    rnodes: Query<&crate::bridge::ReactNode>,
) {
    let Ok((window_entity, current)) = windows.single() else {
        return;
    };

    // Virtual-pointer UI wins: a feature's virtual pointer (a `<surface>`'s
    // in-world ray-cast) is the only thing that knows the pointer is over an
    // offscreen subtree. Then the transform3d remap, then the mouse itself.
    let named = hover_map.as_deref().and_then(|hover_map| {
        virtual_pointers
            .iter()
            .flat_map(|pointers| pointers.iter())
            .chain(transform3d_pointer.as_deref().map(|pointer| pointer.id))
            .chain([PointerId::Mouse])
            .find_map(|id| hovered_cursor(id, hover_map, &node_cursors, &child_of))
    });

    // No cursor-bearing node under the pointer → the app default. Routing it through
    // `resolve_cursor("default", …)` (rather than a hardcoded arrow) lets a custom
    // cursor registered as `"default"` become the app-wide base cursor.
    let desired = {
        // Attribute an unknown-cursor warning to the node that carried the
        // `cursor` style, so devtools can flag its row.
        let _diag = named
            .as_ref()
            .and_then(|(_, e)| rnodes.get(*e).ok())
            .map(|r| crate::diag::node_scope(r.0));
        resolve_cursor(
            named.as_ref().map(|(n, _)| n.as_str()).unwrap_or("default"),
            &custom_cursors,
        )
    };
    if current != Some(&desired) {
        commands.entity(window_entity).insert(desired);
    }
}

/// The cursor a pointer's hover asks for (name + the entity that carried it):
/// of the nodes it hovers, the topmost whose own or nearest ancestor's
/// `NodeCursor` exists. `None` when it hovers nothing, or nothing with a
/// cursor.
fn hovered_cursor(
    pointer_id: PointerId,
    hover_map: &HoverMap,
    node_cursors: &Query<&NodeCursor>,
    child_of: &Query<&ChildOf>,
) -> Option<(String, Entity)> {
    // Topmost = smallest picking depth (the UI backend assigns depth 0 to the
    // front-most node, increasing downward through the stack).
    let (_, owner) = hover_map
        .get(&pointer_id)?
        .iter()
        .filter_map(|(&hovered, hit)| {
            crate::reconcile::climb(hovered, child_of, |e| node_cursors.contains(e))
                .map(|owner| (hit.depth, owner))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
    node_cursors.get(owner).ok().map(|c| (c.0.clone(), owner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::entity::EntityHashMap;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::picking::backend::HitData;

    /// Run `drive_cursor_icon` with the mouse hovering `stack` — topmost
    /// first, `None` a node without a cursor — and return the window's
    /// resulting `CursorIcon` (if any was written).
    fn run(stack: &[Option<&str>]) -> Option<CursorIcon> {
        let mut world = World::new();
        let window_entity = world.spawn((Window::default(), PrimaryWindow)).id();
        let mut hovered = EntityHashMap::default();
        for (i, name) in stack.iter().enumerate() {
            let mut node = world.spawn_empty();
            if let Some(name) = name {
                node.insert(NodeCursor(name.to_string()));
            }
            let depth = i as f32 * 0.00001;
            hovered.insert(
                node.id(),
                HitData::new(Entity::PLACEHOLDER, depth, None, None),
            );
        }
        let mut hover_map = HoverMap::default();
        hover_map.insert(PointerId::Mouse, hovered);
        world.insert_resource(hover_map);
        world.init_resource::<CustomCursors>();

        world.run_system_once(drive_cursor_icon).unwrap();
        world.entity(window_entity).get::<CursorIcon>().cloned()
    }

    #[test]
    fn topmost_node_under_pointer_wins() {
        let icon = run(&[Some("pointer"), Some("grab")]);
        assert_eq!(icon, Some(CursorIcon::from(SystemCursorIcon::Pointer)));
    }

    /// A node without a cursor is transparent: an overlay shows the cursor of
    /// what is beneath it.
    #[test]
    fn a_node_without_a_cursor_shows_the_one_beneath() {
        let icon = run(&[None, Some("text")]);
        assert_eq!(icon, Some(CursorIcon::from(SystemCursorIcon::Text)));
    }

    #[test]
    fn hovering_no_cursor_node_resets_to_default() {
        let stacks: [&[Option<&str>]; 2] = [&[], &[None]];
        for stack in stacks {
            let icon = run(stack);
            assert_eq!(icon, Some(CursorIcon::from(SystemCursorIcon::Default)));
        }
    }

    /// A node clipped away by a scroll container sets no cursor from where it
    /// is laid out: the hover map only holds what is really hit (real layout,
    /// clipping, picking — the `clip_pick_tests` scene).
    #[test]
    fn a_clipped_away_node_does_not_set_the_cursor() {
        use crate::reconcile::clip_pick_tests::{OVER_CLIPPED, OVER_SHOWN, clip_scene};
        let icon = |pointer: Vec2| {
            let mut scene = clip_scene(pointer);
            scene.app.init_resource::<CustomCursors>();
            scene.app.add_systems(Update, drive_cursor_icon);
            let world = scene.app.world_mut();
            world
                .entity_mut(scene.shown)
                .insert(NodeCursor("text".to_string()));
            world
                .entity_mut(scene.clipped)
                .insert(NodeCursor("pointer".to_string()));
            scene.app.update();
            scene.app.update();
            let window = scene.app.world().entity(scene.window);
            window.get::<CursorIcon>().cloned()
        };
        assert_eq!(
            icon(OVER_SHOWN),
            Some(CursorIcon::from(SystemCursorIcon::Text))
        );
        assert_eq!(
            icon(OVER_CLIPPED),
            Some(CursorIcon::from(SystemCursorIcon::Default))
        );
    }

    /// A custom cursor registered under `"default"` becomes the app-wide base cursor:
    /// with no cursor-bearing node under the pointer, the window shows it (not the
    /// hardcoded system arrow).
    #[cfg(feature = "custom_cursor")]
    #[test]
    fn registered_default_becomes_app_cursor() {
        let mut world = World::new();
        // Nothing hovered → nothing supplies a cursor, so the app default is used.
        let window_entity = world.spawn((Window::default(), PrimaryWindow)).id();

        let arrow = CustomCursorImage {
            handle: Handle::default(),
            hotspot: (0, 0),
            ..default()
        };
        let mut registry = CustomCursors::default();
        registry.0.insert("default".into(), arrow.clone());
        world.insert_resource(registry);

        world.run_system_once(drive_cursor_icon).unwrap();
        assert_eq!(
            world.entity(window_entity).get::<CursorIcon>().cloned(),
            Some(CursorIcon::Custom(CustomCursor::Image(arrow))),
        );
    }

    /// A hovered leaf inherits its ancestor's cursor (climb), and the topmost
    /// (smallest-depth) hovered node wins.
    #[test]
    fn hovered_leaf_uses_its_ancestors_cursor() {
        let mut world = World::new();
        // A `<button>`-like parent with a cursor; a childless leaf (e.g. its `<text>`)
        // with none. The pointer hovers the leaf.
        let parent = world.spawn(NodeCursor("pointer".to_string())).id();
        let leaf = world.spawn(ChildOf(parent)).id();
        // A second, deeper node with a different cursor that must lose on depth.
        let behind = world.spawn(NodeCursor("grab".to_string())).id();

        let pointer_id = PointerId::Mouse;
        let mut hovered = EntityHashMap::default();
        hovered.insert(leaf, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
        hovered.insert(behind, HitData::new(Entity::PLACEHOLDER, 1.0, None, None));
        let mut hover_map = HoverMap::default();
        hover_map.insert(pointer_id, hovered);
        world.insert_resource(hover_map);

        let hit = world
            .run_system_once(
                move |hm: Res<HoverMap>, ncs: Query<&NodeCursor>, co: Query<&ChildOf>| {
                    hovered_cursor(pointer_id, &hm, &ncs, &co)
                },
            )
            .unwrap();
        assert_eq!(hit, Some(("pointer".to_string(), parent)));
    }

    /// A hovered node with no cursor on itself or any ancestor contributes
    /// nothing (so the next pointer / the default takes over).
    #[test]
    fn hover_without_a_cursor_yields_none() {
        let mut world = World::new();
        let bare = world.spawn_empty().id();
        let pointer_id = PointerId::Mouse;
        let mut hovered = EntityHashMap::default();
        hovered.insert(bare, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
        let mut hover_map = HoverMap::default();
        hover_map.insert(pointer_id, hovered);
        world.insert_resource(hover_map);

        let hit = world
            .run_system_once(
                move |hm: Res<HoverMap>, ncs: Query<&NodeCursor>, co: Query<&ChildOf>| {
                    hovered_cursor(pointer_id, &hm, &ncs, &co)
                },
            )
            .unwrap();
        assert_eq!(hit, None);
    }

    /// Resolution order: the registry wins first (so a custom cursor named after a
    /// system keyword *overrides* it), then system keywords, then a warn + default.
    #[cfg(feature = "custom_cursor")]
    #[test]
    fn custom_cursor_overrides_and_resolves() {
        let mut registry = CustomCursors::default();
        let hand = CustomCursorImage {
            handle: Handle::default(),
            hotspot: (4, 2),
            ..default()
        };
        registry.0.insert("hand".into(), hand.clone());
        // Register a custom cursor under a SYSTEM keyword name to prove override.
        registry.0.insert("pointer".into(), hand.clone());

        // Registered custom name → custom.
        assert_eq!(
            resolve_cursor("hand", &registry),
            CursorIcon::Custom(CustomCursor::Image(hand.clone())),
        );
        // "pointer" is registered → the custom image overrides the system pointer.
        assert_eq!(
            resolve_cursor("pointer", &registry),
            CursorIcon::Custom(CustomCursor::Image(hand)),
        );
        // A system keyword that isn't registered → the built-in cursor.
        assert_eq!(
            resolve_cursor("text", &registry),
            CursorIcon::from(SystemCursorIcon::Text),
        );
        // Neither registered nor a keyword → warn + default arrow.
        assert_eq!(
            resolve_cursor("missing", &registry),
            CursorIcon::from(SystemCursorIcon::Default),
        );
    }
}
