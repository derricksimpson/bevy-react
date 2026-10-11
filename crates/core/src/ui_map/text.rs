//! Text: font, color, layout, line height, spacing, and shadow resolution.

use super::*;

/// The font size a `<text>` block inherits: nothing above it carries one.
pub(crate) const ROOT_FONT_SIZE: BevyFontSize = BevyFontSize::Rem(1.0);

/// Convert a wire [`FontSize`] into bevy's `FontSize` (the `TextFont` field type),
/// mapping each unit one-to-one (`Rem` resolves against bevy's `RemSize`
/// resource) — but `Em`, which bevy lacks: a multiple of `inherited`, kept in
/// that size's own unit (so an `em` of a `rem` or viewport size stays
/// responsive).
fn font_size_to_bevy(size: FontSize, inherited: BevyFontSize) -> BevyFontSize {
    match size {
        FontSize::Px(v) => BevyFontSize::Px(v),
        FontSize::Vw(v) => BevyFontSize::Vw(v),
        FontSize::Vh(v) => BevyFontSize::Vh(v),
        FontSize::VMin(v) => BevyFontSize::VMin(v),
        FontSize::VMax(v) => BevyFontSize::VMax(v),
        FontSize::Rem(v) => BevyFontSize::Rem(v),
        FontSize::Em(v) => inherited * v,
    }
}

/// Map a [`LineHeightSpec`] to bevy's [`LineHeight`]: a bare number is a multiple
/// of the font size, `{ px }` is an absolute pixel height, and a string carries a
/// unit (`"20px"` absolute, else a multiple).
pub(super) fn line_height(spec: &LineHeightSpec) -> LineHeight {
    match spec {
        LineHeightSpec::Relative(scale) => LineHeight::RelativeToFont(*scale),
        LineHeightSpec::Px { px } => LineHeight::Px(*px),
        LineHeightSpec::Str(s) => {
            let s = s.trim();
            if let Some(n) = s.strip_suffix("px") {
                if let Ok(v) = n.trim().parse() {
                    return LineHeight::Px(v);
                }
            } else {
                // unitless or `em`/`rem` → a multiple of the font size.
                let num = s
                    .strip_suffix("rem")
                    .or_else(|| s.strip_suffix("em"))
                    .unwrap_or(s);
                if let Ok(v) = num.trim().parse() {
                    return LineHeight::RelativeToFont(v);
                }
            }
            let msg = format!("invalid lineHeight {s:?}");
            crate::diag::report("lineHeight", s, &msg);
            LineHeight::default()
        }
    }
}

/// Map a [`LetterSpacingSpec`] to bevy's [`LetterSpacing`]: a bare number is
/// logical pixels, `{ rem }` is a multiple of the root size, and a string
/// carries a unit (`"2px"`, `"0.1rem"`, `"0.1em"`, or `"normal"`). `em` is a
/// multiple of the run's own `font_size`, as in CSS.
pub(super) fn letter_spacing(spec: &LetterSpacingSpec, font_size: BevyFontSize) -> LetterSpacing {
    match spec {
        LetterSpacingSpec::Px(px) => LetterSpacing::Px(*px),
        LetterSpacingSpec::Rem { rem } => LetterSpacing::Rem(*rem),
        LetterSpacingSpec::Str(s) => {
            let s = s.trim();
            if s.eq_ignore_ascii_case("normal") {
                return LetterSpacing::default();
            }
            if let Some(n) = s.strip_suffix("px") {
                if let Ok(v) = n.trim().parse() {
                    return LetterSpacing::Px(v);
                }
            } else if let Some(n) = s.strip_suffix("rem") {
                if let Ok(v) = n.trim().parse() {
                    return LetterSpacing::Rem(v);
                }
            } else if let Some(n) = s.strip_suffix("em") {
                if let Ok(v) = n.trim().parse::<f32>() {
                    return match font_size {
                        BevyFontSize::Px(px) => LetterSpacing::Px(v * px),
                        BevyFontSize::Rem(rem) => LetterSpacing::Rem(v * rem),
                        // bevy has no letter spacing in viewport units.
                        _ => {
                            let msg = format!(
                                "letterSpacing {s:?} on a viewport-unit fontSize \
                                 reads as rem (use px or rem spacing)"
                            );
                            crate::diag::report("letterSpacing", s, &msg);
                            LetterSpacing::Rem(v)
                        }
                    };
                }
            } else if let Ok(v) = s.parse() {
                return LetterSpacing::Px(v); // bare numeric string → logical pixels
            }
            let msg = format!("invalid letterSpacing {s:?}");
            crate::diag::report("letterSpacing", s, &msg);
            LetterSpacing::default()
        }
    }
}

/// Build a [`TextShadow`] from a style's `textShadow`, folding `opacity` (as
/// resolved by the caller — `None` on a promoted layer root) into the color.
/// Unset offset/color fields fall back to bevy's [`TextShadow::default`].
pub(crate) fn text_shadow(style: Option<&Style>, opacity: Option<f32>) -> Option<TextShadow> {
    let s = style?;
    let spec = s.get(&TEXT_SHADOW)?;
    let mut shadow = TextShadow::default();
    if let Some(x) = spec.offset_x {
        shadow.offset.x = x;
    }
    if let Some(y) = spec.offset_y {
        shadow.offset.y = y;
    }
    if let Some(c) = &spec.color {
        shadow.color = parse_color(c);
    }
    shadow.color = apply_opacity(shadow.color, opacity);
    Some(shadow)
}

/// Resolve a style's text appearance into the `TextColor` + `TextFont` +
/// `LineHeight` + `LetterSpacing` a text run carries. Unset properties fall
/// back to white / Bevy's defaults. Returned as concrete components so they
/// can be copied onto inheriting child spans. On a `promoted` text root the
/// `opacity` fold into the glyph color is suppressed — the value drives the
/// layer's composite-time group alpha instead (without this, a promoted
/// `<text>` would fade twice).
pub fn resolved_text_style(
    style: Option<&Style>,
    fonts: &Fonts,
    promoted: bool,
) -> (TextColor, TextFont, LineHeight, LetterSpacing) {
    let (font, line, spacing) = resolve_text_font(style, fonts);
    (resolve_text_color(style, promoted), font, line, spacing)
}

/// The color half of the resolved text style: `color` (default white) with
/// the `opacity` fold — suppressed on a promoted root.
pub(crate) fn resolve_text_color(style: Option<&Style>, promoted: bool) -> TextColor {
    let mut color = TextColor(Color::WHITE);
    if let Some(s) = style {
        if let Some(c) = s.get(&COLOR).static_ref() {
            color = TextColor(parse_color(c));
        }
        if s.get(&OPACITY).is_some() && !promoted {
            color = TextColor(apply_opacity(color.0, s.get(&OPACITY).static_val()));
        }
    }
    color
}

/// The shaping half of the resolved text style: `TextFont` (size, weight,
/// family — the configured default face when unset) + `LineHeight` +
/// `LetterSpacing`. An `em` font size is root-relative here — right for a
/// block; a run inside another resolves through [`resolve_text_font_in`].
pub(crate) fn resolve_text_font(
    style: Option<&Style>,
    fonts: &Fonts,
) -> (TextFont, LineHeight, LetterSpacing) {
    resolve_text_font_in(style, fonts, ROOT_FONT_SIZE)
}

/// [`resolve_text_font`] for a run inside another: an `em` font size is a
/// multiple of `inherited`, the enclosing run's resolved size.
pub(crate) fn resolve_text_font_in(
    style: Option<&Style>,
    fonts: &Fonts,
    inherited: BevyFontSize,
) -> (TextFont, LineHeight, LetterSpacing) {
    let mut font = TextFont::default();
    let mut line = LineHeight::default();
    let mut spacing = LetterSpacing::default();
    // Default font face; a `fontFamily` below overrides it. Unset on both → leave
    // `TextFont::default()`'s empty handle (Bevy's built-in font).
    if let Some(h) = &fonts.default {
        font.font = FontSource::Handle(h.clone());
    }
    if let Some(s) = style {
        if let Some(size) = s.get(&FONT_SIZE).copied() {
            font.font_size = font_size_to_bevy(size, inherited);
        }
        if let Some(w) = s.get(&FONT_WEIGHT).copied() {
            font.weight = w;
        }
        if let Some(family) = s.get(&FONT_FAMILY) {
            font.font = match fonts.named.get(family) {
                Some(h) => FontSource::Handle(h.clone()),
                None => font_list(family, fonts, font.font),
            };
        }
        if let Some(lh) = s.get(&LINE_HEIGHT) {
            line = line_height(lh);
        }
        if let Some(ls) = s.get(&LETTER_SPACING) {
            // After `fontSize`: `em` spacing is a multiple of it.
            spacing = letter_spacing(ls, font.font_size);
        }
    }
    (font, line, spacing)
}

/// The CSS generic font families, by keyword. Bevy resolves them through the
/// font database: the system's fonts under its `system_font_discovery`
/// feature, or whatever the app mapped (`FontCx::set_sans_serif_family`, …).
const GENERIC_FAMILIES: &[(&str, GenericFontFamily)] = &[
    ("serif", GenericFontFamily::Serif),
    ("sans-serif", GenericFontFamily::SansSerif),
    ("cursive", GenericFontFamily::Cursive),
    ("fantasy", GenericFontFamily::Fantasy),
    ("monospace", GenericFontFamily::Monospace),
    ("system-ui", GenericFontFamily::SystemUi),
    ("ui-serif", GenericFontFamily::UiSerif),
    ("ui-sans-serif", GenericFontFamily::UiSansSerif),
    ("ui-monospace", GenericFontFamily::UiMonospace),
    ("ui-rounded", GenericFontFamily::UiRounded),
    ("emoji", GenericFontFamily::Emoji),
    ("math", GenericFontFamily::Math),
    ("fangsong", GenericFontFamily::FangSong),
];

/// A `fontFamily` that is not one registered name: a CSS fallback list
/// (`"Display, Inter, sans-serif"`), tried per glyph in order. An entry is a
/// registered name (its font) or a generic keyword; anything else reports
/// `fontFamily` and is skipped — a system font is not reachable by name. The
/// `default` face closes the list, so a list nothing in matched (the single
/// unknown name included) is the default face itself and text never ends up
/// without a font.
fn font_list(list: &str, fonts: &Fonts, default: FontSource) -> FontSource {
    let mut sources = Vec::new();
    for name in list.split(',') {
        let name = name.trim().trim_matches(['"', '\'']);
        if let Some(h) = fonts.named.get(name) {
            sources.push(FontSource::Handle(h.clone()));
        } else if let Some((_, generic)) = GENERIC_FAMILIES
            .iter()
            .find(|(keyword, _)| name.eq_ignore_ascii_case(keyword))
        {
            sources.push(FontSource::Generic(*generic));
        } else {
            let msg = format!("unknown fontFamily {name:?}");
            crate::diag::report("fontFamily", name, &msg);
        }
    }
    if !sources.contains(&default) {
        sources.push(default);
    }
    match sources.len() {
        1 => sources.remove(0),
        _ => FontSource::List(sources),
    }
}

/// Land a resolved text style on a text root or an inheriting span, writing
/// only the halves asked for — and each component compare-before-write
/// ([`set_if_neq_or_insert`]), so an unchanged `TextFont` never ticks. This
/// is what keeps a `color`-only delta from re-shaping the block: bevy_text's
/// `detect_text_needs_rerender` keys on `Changed<TextFont | LineHeight |
/// LetterSpacing>`, and a re-shape rebuilds the measure func →
/// `ContentSize` → a taffy relayout of the whole tree. The spawn paths insert
/// the whole tuple directly (nothing to compare against).
pub fn apply_resolved_text_style(
    ec: &mut EntityCommands,
    resolved: &crate::bridge::ResolvedTextStyle,
    color_half: bool,
    font_half: bool,
) {
    let (color, font, line, spacing) = resolved;
    if color_half {
        ec.queue(set_if_neq_or_insert(*color));
    }
    if font_half {
        ec.queue(set_if_neq_or_insert(font.clone()));
        ec.queue(set_if_neq_or_insert(*line));
        ec.queue(set_if_neq_or_insert(*spacing));
    }
}

/// The `TextLayout` for a `<text>` root, if `textAlign` or `lineBreak` is set
/// (root only). Either property present builds the layout; the other keeps
/// its bevy default.
pub fn text_layout(style: Option<&Style>) -> Option<TextLayout> {
    let s = style?;
    if s.get(&TEXT_ALIGN).is_none() && s.get(&LINE_BREAK).is_none() {
        return None;
    }
    Some(TextLayout {
        justify: s.get(&TEXT_ALIGN).copied().unwrap_or_default(),
        linebreak: s.get(&LINE_BREAK).copied().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A default face and two named ones, each its own handle.
    fn fonts() -> (Fonts, [Handle<Font>; 3]) {
        let assets = Assets::<Font>::default();
        let handles = [(); 3].map(|_| assets.reserve_handle());
        let [default, display, mono] = handles.clone();
        let named = [("Display".to_owned(), display), ("Mono".to_owned(), mono)].into();
        (
            Fonts {
                default: Some(default),
                named,
            },
            handles,
        )
    }

    fn source(family: &str, fonts: &Fonts) -> FontSource {
        let style: Style =
            serde_json::from_value(serde_json::json!({ "fontFamily": family })).unwrap();
        resolve_text_font(Some(&style), fonts).0.font
    }

    /// One registered name is that font alone, as it always was; a comma
    /// list is a `FontSource::List` in the written order — registered names
    /// (quoted or not) as their handles, generic keywords as bevy's generic
    /// families — closed by the default face, once.
    #[test]
    fn font_family_list_maps_to_a_font_source_list() {
        let (fonts, [default, display, mono]) = fonts();
        let handle = |h: &Handle<Font>| FontSource::Handle(h.clone());
        assert_eq!(source("Display", &fonts), handle(&display));
        assert_eq!(
            source("Display, 'Mono'", &fonts),
            FontSource::List(vec![handle(&display), handle(&mono), handle(&default)])
        );
        assert_eq!(
            source("Mono, Sans-Serif", &fonts),
            FontSource::List(vec![
                handle(&mono),
                FontSource::Generic(GenericFontFamily::SansSerif),
                handle(&default),
            ])
        );

        // The default face under a registered name is not listed twice.
        let mut fonts = fonts;
        fonts.named.insert("Body".to_owned(), default.clone());
        assert_eq!(
            source("Display, Body", &fonts),
            FontSource::List(vec![handle(&display), handle(&default)])
        );
    }

    /// An unknown name is reported and skipped, never passed to the font
    /// database; a value nothing in matched is the default face itself
    /// (what a single unknown name always resolved to).
    #[test]
    fn unknown_font_family_entries_warn_and_are_skipped() {
        let _lock = crate::diag::test_lock();
        let (fonts, [default, display, _]) = fonts();
        let handle = |h: &Handle<Font>| FontSource::Handle(h.clone());
        crate::diag::arm_runtime();
        let _scope = crate::diag::node_scope(4343);
        assert_eq!(source("Nope", &fonts), handle(&default));
        assert_eq!(
            source("Nope, Display", &fonts),
            FontSource::List(vec![handle(&display), handle(&default)])
        );
        #[cfg(all(feature = "devtools", debug_assertions))]
        {
            let unknown = crate::diag::take_runtime_warnings()
                .into_iter()
                .filter(|w| w.node == Some(4343))
                .map(|w| (w.kind, w.value))
                .collect::<Vec<_>>();
            assert_eq!(unknown, vec![("fontFamily", "Nope".to_owned()); 2]);
        }
    }
}
