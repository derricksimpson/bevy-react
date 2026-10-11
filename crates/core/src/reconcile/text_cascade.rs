//! Text style inheritance. Bevy has none, so the op path resolves a text
//! node's style once (cached in [`JsBridge::text_styles`]) and pushes it down
//! itself: a bare-string run copies its parent's resolved style, a nested
//! `<text>` keeps its own — except a `fontSize` in `em`, which is a multiple
//! of the enclosing run's size and is re-resolved whenever that changes.

use bevy::prelude::*;
use bevy::text::FontSize as BevyFontSize;

use crate::bridge::{JsBridge, ResolvedTextStyle, SpanKind};
use crate::plugin::Fonts;
use crate::protocol::{NodeId, units::FontSize};
use crate::style::Style;
use crate::style::props::FONT_SIZE;
use crate::ui_map::{apply_resolved_text_style, resolve_text_font_in};

/// Push text node `id`'s `resolved` style to what inherits from it: every
/// bare-string run copies the halves asked for; with `font_half`, every
/// nested `<text>` sized in `em` re-resolves against it ([`rebase`]) and
/// cascades on. `rebased` collects the re-resolved spans for the caller to
/// write into [`JsBridge::text_styles`] — or to drop, when `resolved` is a
/// hover/press overlay the cache must not keep.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cascade(
    commands: &mut Commands,
    bridge: &JsBridge,
    fonts: &Fonts,
    id: NodeId,
    resolved: &ResolvedTextStyle,
    color_half: bool,
    font_half: bool,
    rebased: &mut Vec<(NodeId, ResolvedTextStyle)>,
) {
    for kid in bridge.children_of(id) {
        match bridge.spans.get(&kid) {
            Some(SpanKind::RawInherited) => {
                if let Some(&entity) = bridge.nodes.get(&kid) {
                    apply_resolved_text_style(
                        &mut commands.entity(entity),
                        resolved,
                        color_half,
                        font_half,
                    );
                }
            }
            Some(SpanKind::InlineStyled) if font_half => {
                rebase(commands, bridge, fonts, kid, resolved.1.font_size, rebased);
            }
            _ => {}
        }
    }
}

/// Whether a style's `fontSize` is in `em` — the one thing of a nested
/// `<text>` that depends on its parent.
pub(super) fn em_sized(style: Option<&Style>) -> bool {
    style.is_some_and(|s| matches!(s.get(&FONT_SIZE), Some(FontSize::Em(_))))
}

/// Re-resolve styled span `id` against `inherited`, the enclosing run's font
/// size, if its own is in `em`, and cascade on to what inherits from it.
pub(crate) fn rebase(
    commands: &mut Commands,
    bridge: &JsBridge,
    fonts: &Fonts,
    id: NodeId,
    inherited: BevyFontSize,
    rebased: &mut Vec<(NodeId, ResolvedTextStyle)>,
) {
    let style = bridge.props_cache.get(&id).and_then(|p| p.style.as_ref());
    if !em_sized(style) {
        return;
    }
    let (Some(&entity), Some(cached)) = (bridge.nodes.get(&id), bridge.text_styles.get(&id)) else {
        return;
    };
    let (font, line, spacing) = resolve_text_font_in(style, fonts, inherited);
    let own = (cached.0, font, line, spacing);
    apply_resolved_text_style(&mut commands.entity(entity), &own, false, true);
    cascade(commands, bridge, fonts, id, &own, false, true, rebased);
    rebased.push((id, own));
}

#[cfg(test)]
mod tests {
    use super::super::test_util::{create, ent, op_app, update};
    use super::*;
    use crate::bridge::ReactInteraction;
    use crate::protocol::{ROOT_ID, op::Op};
    use serde_json::json;

    fn append(parent: NodeId, child: NodeId) -> Op {
        Op::Append { parent, child }
    }

    fn run(id: NodeId) -> Op {
        Op::CreateTextSpan {
            id,
            text: "run".into(),
        }
    }

    fn font_size(app: &App, id: NodeId) -> BevyFontSize {
        let font = app.world().entity(ent(app, id)).get::<TextFont>();
        font.expect("a text font").font_size
    }

    /// Text 1 (30px) holding span 2 (`0.5em`), which holds bare run 3 and
    /// span 4 (`2em`) — attached bottom-up, as React mounts a tree.
    fn nested() -> (App, crossbeam_channel::Sender<Vec<Op>>) {
        let (mut app, tx) = op_app();
        tx.send(vec![
            create(1, "text", json!({ "style": { "fontSize": 30 } })),
            create(2, "textSpan", json!({ "style": { "fontSize": "0.5em" } })),
            run(3),
            create(4, "textSpan", json!({ "style": { "fontSize": "2em" } })),
            append(2, 3),
            append(2, 4),
            append(1, 2),
            append(ROOT_ID, 1),
        ])
        .unwrap();
        app.update();
        (app, tx)
    }

    /// An `em` span is a multiple of the text it sits in — and so, down the
    /// tree, are the runs and `em` spans inside it, whatever was attached
    /// first.
    #[test]
    fn em_span_resolves_against_its_parent() {
        let (mut app, tx) = nested();
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(15.0));
        assert_eq!(font_size(&app, 3), BevyFontSize::Px(15.0), "its run");
        assert_eq!(font_size(&app, 4), BevyFontSize::Px(30.0), "its em span");

        // The cache holds the re-resolved size: a run attached later copies it.
        tx.send(vec![run(5), append(2, 5)]).unwrap();
        app.update();
        assert_eq!(font_size(&app, 5), BevyFontSize::Px(15.0));
    }

    /// A parent's font delta re-sizes its `em` spans, all the way down.
    #[test]
    fn em_span_follows_parent_delta() {
        let (mut app, tx) = nested();
        tx.send(vec![update(
            1,
            "text",
            json!({ "style": { "fontSize": 40 } }),
            &[],
            &[],
        )])
        .unwrap();
        app.update();
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(20.0));
        assert_eq!(font_size(&app, 3), BevyFontSize::Px(20.0));
        assert_eq!(font_size(&app, 4), BevyFontSize::Px(40.0));
    }

    /// A span's own delta resolves against its parent; once it is no longer
    /// in `em`, the parent's deltas leave it alone.
    #[test]
    fn em_span_own_delta() {
        let (mut app, tx) = nested();
        let restyle = |size: serde_json::Value| {
            update(
                2,
                "textSpan",
                json!({ "style": { "fontSize": size } }),
                &[],
                &[],
            )
        };
        tx.send(vec![restyle(json!("0.75em"))]).unwrap();
        app.update();
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(22.5));
        assert_eq!(font_size(&app, 3), BevyFontSize::Px(22.5), "its run");
        assert_eq!(font_size(&app, 4), BevyFontSize::Px(45.0), "its em span");

        tx.send(vec![restyle(json!(12))]).unwrap();
        app.update();
        tx.send(vec![update(
            1,
            "text",
            json!({ "style": { "fontSize": 40 } }),
            &[],
            &[],
        )])
        .unwrap();
        app.update();
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(12.0));
        assert_eq!(font_size(&app, 4), BevyFontSize::Px(24.0));
    }

    /// `em` keeps the parent's unit: on a block it is root-relative, and a
    /// span inside follows `RemSize` with no cascade at all.
    #[test]
    fn em_stays_in_the_parent_unit() {
        let (mut app, tx) = op_app();
        tx.send(vec![
            create(1, "text", json!({ "style": { "fontSize": "1.5em" } })),
            create(2, "textSpan", json!({ "style": { "fontSize": "0.5em" } })),
            append(1, 2),
            append(ROOT_ID, 1),
        ])
        .unwrap();
        app.update();
        assert_eq!(font_size(&app, 1), BevyFontSize::Rem(1.5));
        assert_eq!(font_size(&app, 2), BevyFontSize::Rem(0.75));
    }

    /// A run attached in the same batch as its text's font delta takes the
    /// new size (React places children before it updates the parent; the
    /// ECS hierarchy does not show the run yet, the shadow tree does).
    #[test]
    fn run_appended_before_a_same_batch_delta_follows_it() {
        let (mut app, tx) = op_app();
        tx.send(vec![
            create(1, "text", json!({ "style": { "fontSize": 30 } })),
            append(ROOT_ID, 1),
        ])
        .unwrap();
        app.update();
        tx.send(vec![
            run(2),
            append(1, 2),
            update(1, "text", json!({ "style": { "fontSize": 40 } }), &[], &[]),
        ])
        .unwrap();
        app.update();
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(40.0));
    }

    /// A hover variant that re-sizes a block re-sizes its `em` spans with it
    /// and gives them back on hover-out (the cache keeps the base size).
    #[test]
    fn em_span_follows_a_hover_font_size() {
        let (mut app, tx) = op_app();
        app.add_systems(
            Update,
            crate::reconcile::apply_interaction_styles.after(crate::reconcile::apply_js_ops),
        );
        tx.send(vec![
            create(
                1,
                "text",
                json!({ "style": { "fontSize": 30 }, "hoverStyle": { "fontSize": 40 } }),
            ),
            create(2, "textSpan", json!({ "style": { "fontSize": "0.5em" } })),
            append(1, 2),
            append(ROOT_ID, 1),
        ])
        .unwrap();
        app.update();
        let block = ent(&app, 1);
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(15.0));

        app.world_mut()
            .entity_mut(block)
            .insert(ReactInteraction::Hovered);
        app.update();
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(20.0));

        app.world_mut()
            .entity_mut(block)
            .insert(ReactInteraction::None);
        app.update();
        assert_eq!(font_size(&app, 2), BevyFontSize::Px(15.0));
    }
}
