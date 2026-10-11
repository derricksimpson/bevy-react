use bevy::ecs::system::RunSystemOnce;
use bevy::text::{InlineBox, TextElement};
use bevy::ui::widget::TextUiReader;
use serde_json::json;

use super::*;
use crate::protocol::{NodeId, ROOT_ID, op::Op};
use crate::reconcile::test_util::{children_of, create, ent, op_app, update};

fn append(parent: NodeId, child: NodeId) -> Op {
    Op::Append { parent, child }
}

fn run(id: NodeId, text: &str) -> Op {
    Op::CreateTextSpan {
        id,
        text: text.into(),
    }
}

fn image(app: &App, id: NodeId) -> InlineImage {
    let image = app.world().entity(ent(app, id)).get::<InlineImage>();
    image.expect("an InlineImage").clone()
}

/// The block's text tree as bevy walks it: `(entity, depth, is a box)`.
fn walk(app: &mut App, block: Entity) -> Vec<(Entity, usize, bool)> {
    app.world_mut()
        .run_system_once(move |mut reader: TextUiReader| {
            reader
                .iter(block)
                .map(|(e, depth, element)| (e, depth, matches!(element, TextElement::Box(_))))
                .collect::<Vec<_>>()
        })
        .unwrap()
}

/// Text 1 holding run 2, image 3 and run 4.
fn sentence(props: serde_json::Value) -> (App, crossbeam_channel::Sender<Vec<Op>>) {
    let (mut app, tx) = op_app();
    tx.send(vec![
        create(1, "text", json!({})),
        run(2, "a "),
        create(3, "inlineImage", props),
        run(4, " b"),
        append(1, 2),
        append(1, 3),
        append(1, 4),
        append(ROOT_ID, 1),
    ])
    .unwrap();
    app.update();
    (app, tx)
}

/// An inline image is a `Node`-less `InlineImage` among its block's runs, in
/// `Children` order — where bevy's own text walk finds it as a box.
#[test]
fn mounts_as_an_inline_box_between_the_runs() {
    let (mut app, _tx) = sentence(json!({
        "src": "icon.png",
        "tint": "#ff0000",
        "flipX": true,
        "style": { "width": 16, "height": "12px" },
    }));
    let [block, a, img, b] = [1, 2, 3, 4].map(|id| ent(&app, id));
    assert_eq!(children_of(&app, block), [a, img, b]);

    let entity = app.world().entity(img);
    assert!(entity.contains::<InlineBox>() && !entity.contains::<Node>());
    assert!(entity.get::<ElementFlags>().unwrap().node_less);
    let image = image(&app, 3);
    assert_eq!((image.width, image.height), (Some(16.0), Some(12.0)));
    assert!(image.flip_x && !image.flip_y);
    assert_eq!(image.color, Color::srgb(1.0, 0.0, 0.0));
    assert_ne!(image.image, Handle::default(), "`src` is loaded");

    assert_eq!(
        walk(&mut app, block),
        [
            (block, 0, false),
            (a, 1, false),
            (img, 1, true),
            (b, 1, false)
        ]
    );
}

/// Inside a nested `<text>` it is a box of that span's level.
#[test]
fn mounts_inside_a_span() {
    let (mut app, tx) = op_app();
    tx.send(vec![
        create(1, "text", json!({})),
        create(2, "textSpan", json!({})),
        create(3, "inlineImage", json!({})),
        append(2, 3),
        append(1, 2),
        append(ROOT_ID, 1),
    ])
    .unwrap();
    app.update();
    let [block, span, img] = [1, 2, 3].map(|id| ent(&app, id));
    assert_eq!(
        walk(&mut app, block),
        [(block, 0, false), (span, 1, false), (img, 2, true)]
    );
    // No `src`: a solid box in the tint (white), like `<image>`.
    assert_eq!(image(&app, 3).image, Handle::default());
}

/// A delta rebuilds the image from the merged props; one that changes
/// nothing ticks nothing (a tick re-extracts the whole block).
#[test]
fn deltas_rewrite_only_what_changed() {
    let (mut app, tx) = sentence(json!({
        "src": "icon.png",
        "tint": "#ff0000",
        "style": { "width": 16, "height": 12 },
    }));
    let delta = |props, unset: &[&str], style_unset: &[&str]| {
        vec![update(3, "inlineImage", props, unset, style_unset)]
    };
    tx.send(delta(
        json!({ "tint": "#0000ff", "style": { "opacity": 0.5 } }),
        &[],
        &[],
    ))
    .unwrap();
    app.update();
    assert_eq!(image(&app, 3).color, Color::srgba(0.0, 0.0, 1.0, 0.5));

    tx.send(delta(json!({}), &["src"], &["width"])).unwrap();
    app.update();
    let now = image(&app, 3);
    assert_eq!(now.image, Handle::default());
    assert_eq!((now.width, now.height), (None, Some(12.0)));

    let tick = |app: &App| {
        let ticks = app.world().entity(ent(app, 3));
        ticks.get_change_ticks::<InlineImage>().unwrap().changed
    };
    let before = tick(&app);
    tx.send(delta(json!({ "tint": "#0000ff" }), &[], &[]))
        .unwrap();
    app.update();
    assert_eq!(tick(&app), before, "an equal rebuild must not tick");
}

/// The image moves and leaves like any child: bevy re-shapes the block on
/// `Changed<Children>`.
#[test]
fn inserts_before_a_run_and_unmounts() {
    let (mut app, tx) = sentence(json!({}));
    let [block, a, img, b] = [1, 2, 3, 4].map(|id| ent(&app, id));
    tx.send(vec![Op::Insert {
        parent: 1,
        child: 3,
        before: 2,
    }])
    .unwrap();
    app.update();
    assert_eq!(children_of(&app, block), [img, a, b]);

    tx.send(vec![Op::Remove {
        parent: 1,
        child: 3,
    }])
    .unwrap();
    app.update();
    assert_eq!(children_of(&app, block), [a, b]);
    assert!(app.world().get_entity(img).is_err());
}

/// With both sides given the box is in the flow before the texture exists,
/// and its arrival re-shapes nothing; with one side it waits for the
/// texture's aspect ratio, as bevy leaves it.
#[test]
fn both_sides_reserve_the_box_before_the_texture_loads() {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    use bevy::text::InlineBoxKind;
    use bevy::ui::widget::update_inline_image_boxes;
    let (mut app, tx) = op_app();
    app.add_systems(
        PostUpdate,
        (update_inline_image_boxes, reserve_inline_image_boxes).chain(),
    );
    tx.send(vec![
        create(1, "text", json!({})),
        create(
            2,
            "inlineImage",
            json!({ "style": { "width": 16, "height": 12 } }),
        ),
        create(3, "inlineImage", json!({ "style": { "width": 16 } })),
        append(1, 2),
        append(1, 3),
        append(ROOT_ID, 1),
    ])
    .unwrap();
    app.update();
    // Point both at a texture that is not loaded yet.
    let texture = app.world().resource::<Assets<Image>>().reserve_handle();
    for id in [2, 3] {
        let entity = ent(&app, id);
        let mut image = app.world_mut().get_mut::<InlineImage>(entity).unwrap();
        image.image = texture.clone();
    }
    app.update();
    let inline_box = |app: &App, id| *app.world().get::<InlineBox>(ent(app, id)).unwrap();
    let in_flow = |x, y| InlineBox {
        kind: InlineBoxKind::InFlow,
        size: Vec2::new(x, y),
    };
    assert_eq!(inline_box(&app, 2), in_flow(16.0, 12.0), "reserved");
    assert_eq!(inline_box(&app, 3), InlineBox::default(), "waits");

    let tick = |app: &App| {
        let ticks = app.world().entity(ent(app, 2));
        ticks.get_change_ticks::<InlineBox>().unwrap().changed
    };
    let reserved = tick(&app);
    let size = Extent3d {
        width: 4,
        height: 2,
        depth_or_array_layers: 1,
    };
    let format = TextureFormat::Rgba8UnormSrgb;
    let pixels = Image::new_fill(size, TextureDimension::D2, &[0; 4], format, default());
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(texture.id(), pixels)
        .unwrap();
    // `AssetChanged` sees the arrival once the asset events are in.
    app.update();
    app.update();
    assert_eq!(inline_box(&app, 3), in_flow(16.0, 8.0), "the aspect ratio");
    assert_eq!(
        tick(&app),
        reserved,
        "the arrival must not re-shape the line"
    );
}

/// An `em` side is a multiple of the font size of the run the image sits
/// in, a `rem` side of the root's, and both follow it; back in px, the
/// resolver lets go.
#[test]
fn font_relative_sides_follow_the_text() {
    use bevy::text::RemSize;
    let (mut app, tx) = op_app();
    app.init_resource::<RemSize>();
    app.add_systems(PostUpdate, resolve_font_relative_sizes);
    let sized = |id, style| create(id, "inlineImage", json!({ "style": style }));
    tx.send(vec![
        create(1, "text", json!({ "style": { "fontSize": 20 } })),
        create(2, "textSpan", json!({ "style": { "fontSize": "0.5em" } })),
        sized(3, json!({ "width": "2rem", "height": "1.5em" })),
        sized(4, json!({ "height": "1.5em" })),
        append(2, 4),
        append(1, 2),
        append(1, 3),
        append(ROOT_ID, 1),
    ])
    .unwrap();
    app.update();
    let size = |app: &App, id| {
        let image = image(app, id);
        (image.width, image.height)
    };
    assert_eq!(size(&app, 3), (Some(40.0), Some(30.0)));
    assert_eq!(size(&app, 4), (None, Some(15.0)), "the span's size");

    let font_size = json!({ "style": { "fontSize": 40 } });
    tx.send(vec![update(1, "text", font_size, &[], &[])])
        .unwrap();
    app.update();
    assert_eq!(size(&app, 3), (Some(40.0), Some(60.0)));
    assert_eq!(size(&app, 4), (None, Some(30.0)), "through the em span");

    app.world_mut().resource_mut::<RemSize>().0 = 10.0;
    app.update();
    assert_eq!(size(&app, 3), (Some(20.0), Some(60.0)));

    let px = json!({ "style": { "height": 12 } });
    tx.send(vec![update(3, "inlineImage", px, &[], &["width"])])
        .unwrap();
    app.update();
    assert_eq!(size(&app, 3), (None, Some(12.0)));
    assert!(
        !app.world()
            .entity(ent(&app, 3))
            .contains::<FontRelativeSize>()
    );
}

/// A prop change is paint: it dirties the image entity, which the layer
/// membership maps to the block's layer (every `Children` descendant is a
/// member).
#[test]
fn a_prop_change_is_layer_content_dirt() {
    use crate::layer::LayerContentDirt;
    let (mut app, tx) = sentence(json!({}));
    app.init_resource::<LayerContentDirt>();
    tx.send(vec![update(
        3,
        "inlineImage",
        json!({ "tint": "#00ff00" }),
        &[],
        &[],
    )])
    .unwrap();
    app.update();
    let dirt = &app.world().resource::<LayerContentDirt>().nodes;
    assert!(dirt.contains(&ent(&app, 3)));
}

/// The style properties an inline image cannot take are its `styleIgnored`
/// set; the other node-less elements still warn in their own terms.
#[test]
fn ignored_styles_are_everything_but_its_own() {
    use crate::style::props::MARGIN;
    let registry = crate::ext::core_registry();
    let styles = registry.styles();
    let ignored = |kind: &str| registry.element_info(kind).unwrap().ignored_styles;
    assert!(ignored("inlineImage").intersects(&styles.dirty_of(&[&MARGIN])));
    assert!(!ignored("inlineImage").intersects(&styles.dirty_of(&[&WIDTH, &HEIGHT, &OPACITY])));
    assert!(!ignored("textSpan").any(), "a span's set did not widen");
}

/// What an inline image cannot do says so: handlers and state styles
/// (`propIgnored`), `<image>` attributes bevy's inline image lacks
/// (`unknownProp`), an `.svg` source and a size that is not px
/// (`inlineImage`), every other style property (`styleIgnored`). The runtime
/// sink is process-global: serialize, and filter by our node id.
#[cfg(all(feature = "devtools", debug_assertions))]
#[test]
fn unsupported_props_warn() {
    use crate::protocol::props::Props;
    let _lock = crate::diag::test_lock();
    crate::diag::arm_runtime();
    let _ = crate::diag::take_runtime_warnings();

    let (mut app, tx) = op_app();
    crate::diag::decode_batch_start();
    let props = Props::decode_for(
        "inlineImage",
        json!({
            "src": "icon.svg",
            "onClick": true,
            "hoverStyle": { "opacity": 0.5 },
            "imageMode": "stretch",
            "style": { "width": "50%", "height": 8, "margin": 4 },
        }),
    );
    let decoded = crate::diag::take_decode_warnings();
    let decode_warned =
        |kind: &str, value: &str| decoded.iter().any(|w| w.kind == kind && w.value == value);
    assert!(decode_warned("propIgnored", "onClick"), "{decoded:?}");
    assert!(decode_warned("propIgnored", "hoverStyle"), "{decoded:?}");
    assert!(decode_warned("unknownProp", "imageMode"), "{decoded:?}");

    tx.send(vec![Op::Create {
        id: 7,
        kind: "inlineImage".into(),
        props,
        text: None,
    }])
    .unwrap();
    app.update();
    let mine: Vec<_> = crate::diag::take_runtime_warnings()
        .into_iter()
        .filter(|w| w.node == Some(7))
        .map(|w| (w.kind, w.value))
        .collect();
    for expected in [
        ("inlineImage", "icon.svg"),
        ("inlineImage", "width"),
        ("styleIgnored", "margin"),
    ] {
        assert!(
            mine.iter().any(|(k, v)| (*k, v.as_str()) == expected),
            "missing {expected:?} in {mine:?}"
        );
    }
    assert!(
        !mine.iter().any(|(_, v)| v == "height"),
        "a px size is fine: {mine:?}"
    );
    let image = image(&app, 7);
    assert_eq!(
        image.image, TRANSPARENT_IMAGE_HANDLE,
        "an svg draws nothing"
    );
    assert_eq!((image.width, image.height), (None, Some(8.0)));
}
