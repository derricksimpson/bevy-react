//! Two layout bugs of Bevy 0.19 (its `TextMeasure`, taffy 0.10), re-tested
//! on Bevy 0.20 (taffy 0.14) with real layout ([`layout_app`]); the text
//! cases add bevy's real text measuring with its default font.
//!
//! 1. **A clamped width under-measured its wrapped content — fixed.** A node
//!    with `width: "100%"` + `maxWidth` (or a content-sized one with
//!    `maxWidth`, in a row) was sized for the lines its content takes at the
//!    UNCLAMPED width, then clamped: the content wrapped onto more lines
//!    than its box reserved and spilled over what followed. Two causes, both
//!    gone: taffy 0.10 kept the height it had measured before the clamp (a
//!    standalone probe of both taffy versions reproduces it with plain
//!    boxes — [`clamped_page`] is that tree), and 0.19's `TextMeasure`
//!    answered a max-content probe with the one-line height even when the
//!    width was known. The tests below pin the fix.
//! 2. **A wrap row at a fractional scale factor — still present.** taffy
//!    breaks flex lines with a strict `line_length > available` against a
//!    width it derived from those same items in another summation order, so
//!    float error alone wraps the last item of a `flexWrap` row sized to its
//!    own content (seen at a window scale of 1.1667). Kept as an ignored
//!    reproduction.

use super::layout_tests::layout_app;
use bevy::app::PropagateSet;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ComputedUiRenderTargetInfo, UiGlobalTransform, ui_layout_system};

/// [`layout_app`] plus what bevy's text measuring needs: the default font
/// and the measure system, before layout.
fn text_app() -> App {
    use bevy::text::{DEFAULT_FONT_DATA, Font};
    let mut app = layout_app();
    let mut fonts = Assets::<Font>::default();
    fonts
        .insert(
            AssetId::default(),
            Font::from_bytes(DEFAULT_FONT_DATA.to_vec()),
        )
        .unwrap();
    app.insert_resource(fonts);
    app.init_resource::<bevy::text::LayoutCx>();
    app.add_systems(
        PostUpdate,
        (
            bevy::text::load_font_assets_into_font_collection,
            bevy::ui::widget::measure_text_system,
        )
            .chain()
            // The measure reads the propagated scale factor.
            .after(PropagateSet::<ComputedUiRenderTargetInfo>::default())
            .before(ui_layout_system),
    );
    app
}

/// Enough words to wrap several times at 200 px.
const PROSE: &str = "the quick brown fox jumps over the lazy dog and keeps \
                     running past the fence until the words run out";

fn size(app: &App, e: Entity) -> Vec2 {
    app.world().entity(e).get::<ComputedNode>().unwrap().size
}

fn root(app: &mut App, flex_direction: FlexDirection) -> Entity {
    app.world_mut()
        .spawn(Node {
            width: Val::Vw(100.0),
            height: Val::Vh(100.0),
            flex_direction,
            align_items: AlignItems::FlexStart,
            ..default()
        })
        .id()
}

/// The height [`PROSE`] takes when its box is exactly `width` px wide.
fn wrapped_height(width: f32) -> f32 {
    let mut app = text_app();
    let root = root(&mut app, FlexDirection::Column);
    let text = app
        .world_mut()
        .spawn((
            Text::new(PROSE),
            Node {
                width: Val::Px(width),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    app.update();
    app.update();
    let size = size(&app, text);
    assert!(
        size.y > 60.0,
        "the reference wraps onto several lines: {size}"
    );
    size.y
}

/// The `<root>` default's old reason to be a column: a content-sized child
/// with `maxWidth` and wrapping text, in a ROW. The text is first sized at
/// max-content (one line), then clamped; its height must be the wrapped one.
#[test]
fn text_with_max_width_in_a_row_reserves_its_wrapped_height() {
    let expected = wrapped_height(200.0);
    for on_text in [true, false] {
        let mut app = text_app();
        let root = root(&mut app, FlexDirection::Row);
        let clamp = Node {
            max_width: Val::Px(200.0),
            ..default()
        };
        // The clamp on the text itself, or on a box around it.
        let (card, text) = if on_text {
            let text = app
                .world_mut()
                .spawn((Text::new(PROSE), clamp, ChildOf(root)))
                .id();
            (text, text)
        } else {
            let card = app.world_mut().spawn((clamp, ChildOf(root))).id();
            let text = app
                .world_mut()
                .spawn((Text::new(PROSE), Node::default(), ChildOf(card)))
                .id();
            (card, text)
        };
        app.update();
        app.update();
        assert_eq!(size(&app, text), Vec2::new(200.0, expected), "{on_text}");
        assert_eq!(size(&app, card), Vec2::new(200.0, expected), "{on_text}");
    }
}

/// A wrapping text with an explicit width in an `alignItems: "center"` row
/// (the TODO's civilization and cyberpunk reports: laid out as if one word
/// per line). 0.19's `TextMeasure` answered the row's min-content probe with
/// the min-content height although the width was known.
#[test]
fn text_with_a_width_in_a_centered_row_is_measured_at_that_width() {
    let expected = wrapped_height(400.0);
    let mut app = text_app();
    let root = root(&mut app, FlexDirection::Column);
    let row = Node {
        align_items: AlignItems::Center,
        ..default()
    };
    let row = app.world_mut().spawn((row, ChildOf(root))).id();
    let width = Node {
        width: Val::Px(400.0),
        ..default()
    };
    let text = app
        .world_mut()
        .spawn((Text::new(PROSE), width, ChildOf(row)))
        .id();
    app.update();
    app.update();
    assert_eq!(size(&app, text), Vec2::new(400.0, expected));
    assert_eq!(size(&app, row).y, expected);
}

/// A page in the shape that failed on taffy 0.10 (checked against both
/// versions in a standalone probe): a full-width root column, a card in it,
/// and in the card a `stage` holding `clamped` — `container` with
/// `width: "100%"` + `maxWidth: 400` — above a sibling. Returns
/// `(stage, clamped, below)`.
fn clamped_page(
    app: &mut App,
    stage_direction: FlexDirection,
    container: Node,
) -> (Entity, Entity, Entity) {
    let column = || Node {
        flex_direction: FlexDirection::Column,
        ..default()
    };
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Vw(100.0),
            height: Val::Vh(100.0),
            ..column()
        })
        .id();
    let card = app.world_mut().spawn((column(), ChildOf(root))).id();
    let stage = app
        .world_mut()
        .spawn((
            Node {
                flex_direction: stage_direction,
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    let clamped = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Percent(100.0),
                max_width: Val::Px(400.0),
                ..container
            },
            ChildOf(stage),
        ))
        .id();
    let below = app
        .world_mut()
        .spawn((
            Node {
                height: Val::Px(30.0),
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    (stage, clamped, below)
}

fn top(app: &App, e: Entity) -> f32 {
    let center = app.world().entity(e).get::<UiGlobalTransform>().unwrap();
    center.translation.y - size(app, e).y / 2.0
}

/// The demos' header card: wrapped text under `width: "100%"` + `maxWidth`.
/// The clamped box reserves every line, so what follows starts under it.
#[test]
fn text_under_percent_width_and_max_width_reserves_its_wrapped_height() {
    let expected = wrapped_height(400.0);
    for direction in [FlexDirection::Column, FlexDirection::Row] {
        let mut app = text_app();
        let column = Node {
            flex_direction: FlexDirection::Column,
            ..default()
        };
        let (stage, clamped, below) = clamped_page(&mut app, direction, column);
        let text = app
            .world_mut()
            .spawn((Text::new(PROSE), Node::default(), ChildOf(clamped)))
            .id();
        app.update();
        app.update();
        assert_eq!(
            size(&app, text),
            Vec2::new(400.0, expected),
            "{direction:?}"
        );
        assert_eq!(size(&app, stage).y, expected, "{direction:?}");
        assert_eq!(
            top(&app, below),
            top(&app, stage) + expected,
            "{direction:?}"
        );
    }
}

/// The same pair on a wrapping row of boxes (the cursor demo's swatch grid).
/// taffy 0.10 sized the column `stage` for the two lines the row takes at
/// the unclamped width, and the third spilled over `below`.
#[test]
fn wrap_row_under_percent_width_and_max_width_reserves_its_lines() {
    for direction in [FlexDirection::Column, FlexDirection::Row] {
        let mut app = layout_app();
        let wrap = Node {
            flex_wrap: FlexWrap::Wrap,
            ..default()
        };
        let (stage, grid, below) = clamped_page(&mut app, direction, wrap);
        for _ in 0..12 {
            app.world_mut().spawn((
                Node {
                    width: Val::Px(90.0),
                    height: Val::Px(40.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(grid),
            ));
        }
        app.update();
        // Four 90 px swatches per 400 px line: three lines.
        assert_eq!(size(&app, grid), Vec2::new(400.0, 120.0), "{direction:?}");
        assert_eq!(size(&app, stage).y, 120.0, "{direction:?}");
        assert_eq!(top(&app, below), top(&app, stage) + 120.0, "{direction:?}");
    }
}

/// STILL PRESENT with taffy 0.14 (Bevy 0.20) — the reason the demos' `Radio`
/// does not wrap by default. The row is one line tall and exactly as wide as
/// its items, yet the second item lands on a second line, below the row's
/// own box. About one row in seven does it at this scale, across item
/// counts, widths, gaps and paddings; 1.25 and 1.5 are exact in binary and
/// never do. Un-ignore on a taffy upgrade.
#[test]
#[ignore = "taffy 0.14 still wraps a content-sized row on float error"]
fn content_sized_wrap_row_keeps_one_line_at_a_fractional_scale() {
    let mut app = layout_app();
    // The scale a 2560×1440 monitor without a DPI setting runs at.
    app.insert_resource(bevy::ui::UiScale(1.166_666_6));
    let root = root(&mut app, FlexDirection::Column);
    let row = Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(2.0),
        ..default()
    };
    let row = app.world_mut().spawn((row, ChildOf(root))).id();
    let [first, second] = [(); 2].map(|()| {
        let item = Node {
            width: Val::Px(21.0),
            height: Val::Px(20.0),
            flex_shrink: 0.0,
            ..default()
        };
        app.world_mut().spawn((item, ChildOf(row))).id()
    });
    app.update();
    assert_eq!(
        top(&app, second),
        top(&app, first),
        "the second item wrapped"
    );
}
