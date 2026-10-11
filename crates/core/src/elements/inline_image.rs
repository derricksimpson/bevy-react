//! `inlineImage`: an `<image>` written inside a `<text>` (the reconciler's
//! wire kind for it, as `textSpan` is a nested `<text>`'s) — a `Node`-less
//! inline box in its block's text flow, on bevy's `InlineImage`. It takes the
//! `<image>` attributes bevy's inline image has (`src`, `tint`, `flipX`,
//! `flipY`) and, from its `style`, `width`/`height` (the box: px, or `em`/
//! `rem` — see [`FontRelativeSize`]) and `opacity` (folded into the tint).
//! Its pixels belong to the block: the text layout places it, the block's
//! layer captures it, a click on it is a click on the block.

use bevy::asset::asset_changed::AssetChanged;
use bevy::image::TRANSPARENT_IMAGE_HANDLE;
use bevy::prelude::*;
use bevy::text::{FontSize, InlineBox, InlineBoxKind, RemSize};
use bevy::ui::ComputedUiRenderTargetInfo;
use bevy::ui::widget::InlineImage;

use super::image::{FLIP_X, FLIP_Y, SRC, TINT};
use crate::bridge::ReactNode;
use crate::element::{Attrs, Common, Element, SpawnCtx};
use crate::ext::{ElementFlags, TextRole};
use crate::protocol::animatable::{Animatable, AnimatableField};
use crate::protocol::units::Length;
use crate::style::props::{HEIGHT, OPACITY, WIDTH};
use crate::style::{Style, StyleProperty, Writer, owns};
use crate::ui_map::{image_tint, set_if_neq_or_insert};

/// The sides of an inline image given in `em` (a multiple of the font size
/// of the text run it sits in) or `rem`, which bevy's px-only `InlineImage`
/// cannot carry. Stamped only while a side has one;
/// [`resolve_font_relative_sizes`] keeps the px of those sides current.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct FontRelativeSize {
    width: Option<Length>,
    height: Option<Length>,
}

/// An `<image>` inside a `<text>`. Handlers and state styles are outside its
/// common groups (it is no picking target — the block is), and every style
/// property but the three its writer reads is ignored — both warned.
pub static INLINE_IMAGE: Element = Element {
    flags: ElementFlags {
        text: TextRole::InlineBox,
        ..ElementFlags::NODE_LESS
    },
    attrs: &[&SRC, &TINT, &FLIP_X, &FLIP_Y],
    common: Common::IDENTITY,
    writers: &[&INLINE_IMAGE_WRITER],
    spawn: Some(spawn_inline_image),
    ..Element::new("inlineImage")
};

/// The `InlineImage`, rebuilt from the attributes and the style's size and
/// `opacity` whenever one of them changes, and written compare-before-write:
/// bevy re-shapes the block on a changed box and re-extracts it on a changed
/// image, so an equal rebuild must tick nothing.
pub static INLINE_IMAGE_WRITER: Writer = Writer {
    reads: &[&WIDTH, &HEIGHT, &OPACITY],
    attrs: &[&SRC, &TINT, &FLIP_X, &FLIP_Y],
    writes: &[owns::<InlineImage>],
    apply: |ctx, s, ec| {
        // The spawn built a fresh one.
        if ctx.fresh {
            return;
        }
        let (image, relative) = inline_image(ctx.attrs, Some(s), ctx.assets);
        ec.queue(set_if_neq_or_insert(image));
        match relative {
            Some(relative) => ec.insert(relative),
            None => ec.remove::<FontRelativeSize>(),
        };
    },
};

fn spawn_inline_image(ctx: &mut SpawnCtx) -> Entity {
    let (image, relative) = inline_image(&ctx.props.attrs, ctx.style.as_ref(), ctx.assets);
    let entity = ctx.spawn(image);
    if let Some(relative) = relative {
        ctx.commands.entity(entity).insert(relative);
    }
    entity
}

/// The `InlineImage` of the attributes and style, its `em`/`rem` sides left
/// unset for [`resolve_font_relative_sizes`], which gets them as the second
/// value.
fn inline_image(
    attrs: &Attrs,
    style: Option<&Style>,
    assets: &AssetServer,
) -> (InlineImage, Option<FontRelativeSize>) {
    let image = match attrs.get(&SRC) {
        // A solid `tint` box, like a `src`-less `<image>`.
        None => Handle::default(),
        Some(path) if crate::svg::is_svg_src(path) => {
            crate::diag::report(
                "inlineImage",
                path,
                "an .svg `src` is not drawn on an <image> inside <text>: use a \
                 raster image, or put the <image> beside the text",
            );
            // Keeps a sized box in the flow; bevy draws nothing for it.
            TRANSPARENT_IMAGE_HANDLE
        }
        Some(path) => assets.load(path.clone()),
    };
    let (width, height) = (side(style, &WIDTH), side(style, &HEIGHT));
    let px = |side| match side {
        Some(Length::Px(v)) => Some(v),
        _ => None,
    };
    let relative = |side: Option<Length>| side.filter(|s| !matches!(s, Length::Px(_)));
    let relative = FontRelativeSize {
        width: relative(width),
        height: relative(height),
    };
    let image = InlineImage {
        // Never a layer root: the block's layer owns the group fade.
        color: image_tint(
            Color::WHITE,
            attrs.get(&TINT).map(String::as_str),
            style.and_then(|s| s.get(&OPACITY).static_val()),
            false,
        ),
        image,
        width: px(width),
        height: px(height),
        flip_x: attrs.get(&FLIP_X).copied().unwrap_or(false),
        flip_y: attrs.get(&FLIP_Y).copied().unwrap_or(false),
    };
    (image, (relative != default()).then_some(relative))
}

/// One side of the box as the style gives it: a px, `em` or `rem` length, or
/// `None` (unset or `"auto"`), which follows the texture.
fn side(style: Option<&Style>, side: &'static StyleProperty<Animatable<Length>>) -> Option<Length> {
    match style?.get(side)? {
        Animatable::Static(length @ (Length::Px(_) | Length::Em(_) | Length::Rem(_))) => {
            Some(*length)
        }
        Animatable::Static(Length::Auto) => None,
        _ => {
            let name = side.name;
            crate::diag::report(
                "inlineImage",
                name,
                &format!("`{name}` of an <image> inside <text> takes px, em, rem or \"auto\""),
            );
            None
        }
    }
}

/// Keep the px of every `em`/`rem` side current. `em` follows the font size
/// of the run the image sits in (its parent, a block or a span), which a
/// delta, a hover variant, the root font size or a resized viewport can
/// change: it is read every frame, and written only when it differs.
// ponytail: one parent lookup per font-relative inline image per frame; gate
// on the run's `Changed<TextFont>` if thousands of them show up in a profile.
pub(crate) fn resolve_font_relative_sizes(
    rem: Res<RemSize>,
    mut images: Query<(&FontRelativeSize, &ChildOf, &mut InlineImage)>,
    fonts: Query<&TextFont>,
    child_of: Query<&ChildOf>,
    targets: Query<&ComputedUiRenderTargetInfo>,
) {
    for (sizes, parent, mut image) in &mut images {
        let em = || run_font_px(parent.parent(), &fonts, &child_of, &targets, *rem);
        let px = |side: Option<Length>, current: Option<f32>| match side {
            Some(Length::Em(v)) => em().map(|em| v * em),
            Some(Length::Rem(v)) => Some(v * rem.0),
            _ => current,
        };
        let size = (px(sizes.width, image.width), px(sizes.height, image.height));
        if (image.width, image.height) != size {
            (image.width, image.height) = size;
        }
    }
}

/// The font size of text run `run` in logical px, as bevy's text pipeline
/// evaluates it.
fn run_font_px(
    run: Entity,
    fonts: &Query<&TextFont>,
    child_of: &Query<&ChildOf>,
    targets: &Query<&ComputedUiRenderTargetInfo>,
    rem: RemSize,
) -> Option<f32> {
    let size = fonts.get(run).ok()?.font_size;
    // Only a viewport-relative size needs the block's render target.
    let viewport = match size {
        FontSize::Px(_) | FontSize::Rem(_) => Vec2::ZERO,
        _ => {
            let block = crate::reconcile::climb(run, child_of, |e| targets.contains(e))?;
            targets.get(block).ok()?.logical_size()
        }
    };
    Some(size.eval(viewport, rem))
}

/// Bevy keeps an inline image out of the flow until its texture is loaded,
/// even when both sides are given, so the line re-flows when it arrives.
/// Reserve the box, as the web does for `<img width height>`. Runs right
/// after bevy's `update_inline_image_boxes`, on the same change filter: once
/// the texture is in, bevy writes the same box and nothing ticks.
#[allow(clippy::type_complexity)]
pub(crate) fn reserve_inline_image_boxes(
    images: Res<Assets<Image>>,
    mut boxes: Query<
        (&InlineImage, &mut InlineBox),
        (
            With<ReactNode>,
            Or<(Changed<InlineImage>, AssetChanged<InlineImage>)>,
        ),
    >,
) {
    for (image, mut inline_box) in &mut boxes {
        if let (Some(width), Some(height)) = (image.width, image.height)
            && width > 0.0
            && height > 0.0
            && !images.contains(&image.image)
        {
            inline_box.set_if_neq(InlineBox {
                kind: InlineBoxKind::InFlow,
                size: Vec2::new(width, height),
            });
        }
    }
}

#[cfg(test)]
mod tests;
