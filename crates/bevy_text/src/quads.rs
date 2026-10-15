use bevy_asset::AssetId;
use bevy_color::LinearRgba;
use bevy_ecs::{
    query::{Has, QueryData},
    system::Query,
};
use bevy_image::Image;
use bevy_math::{Rect, Vec2};

use crate::{
    ComputedTextBlock, Strikethrough, StrikethroughColor, TextBackgroundColor, TextColor,
    TextLayoutInfo, Underline, UnderlineColor,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextQuadKind {
    Background,
    Shadow,
    Glyph,
    Decoration,
}

#[derive(Clone, Copy, Debug)]
pub struct TextQuad {
    pub kind: TextQuadKind,
    pub texture: Option<AssetId<Image>>,
    pub rect: Rect,
    pub center: Vec2,
    pub size: Vec2,
    pub color: LinearRgba,
    pub is_alpha_mask: bool,
}

impl TextQuad {
    fn rect(kind: TextQuadKind, center: Vec2, size: Vec2, color: LinearRgba) -> Self {
        Self {
            kind,
            texture: None,
            rect: Rect::default(),
            center,
            size,
            color,
            is_alpha_mask: false,
        }
    }
}

#[derive(QueryData)]
pub struct TextSectionStyle {
    pub color: &'static TextColor,
    pub background: Option<&'static TextBackgroundColor>,
    pub strikethrough: Has<Strikethrough>,
    pub underline: Has<Underline>,
    pub strikethrough_color: Option<&'static StrikethroughColor>,
    pub underline_color: Option<&'static UnderlineColor>,
}

// Emits quads in draw order: backgrounds, shadow glyphs and decorations, glyphs, decorations.
// Positions are in text space: physical pixels, origin top-left, y down. `shadow` is an
// offset in that same space plus a color.
pub fn text_quads(
    out: &mut Vec<TextQuad>,
    layout: &TextLayoutInfo,
    block: &ComputedTextBlock,
    styles: &Query<TextSectionStyle>,
    shadow: Option<(Vec2, LinearRgba)>,
) {
    let style = |section: u32| {
        block
            .entities()
            .get(section as usize)
            .and_then(|e| styles.get(e.entity).ok())
    };

    for run in &layout.run_geometry {
        if let Some(background) = style(run.section_index).and_then(|s| s.background) {
            out.push(TextQuad::rect(
                TextQuadKind::Background,
                run.bounds.center(),
                run.bounds.size(),
                background.0.to_linear(),
            ));
        }
    }

    if let Some((offset, color)) = shadow {
        for glyph in &layout.glyphs {
            out.push(TextQuad {
                kind: TextQuadKind::Shadow,
                texture: Some(glyph.atlas_info.texture),
                rect: glyph.atlas_info.rect,
                center: glyph.position + offset,
                size: glyph.atlas_info.rect.size(),
                color,
                is_alpha_mask: glyph.atlas_info.is_alpha_mask,
            });
        }
        for run in &layout.run_geometry {
            let Some(style) = style(run.section_index) else {
                continue;
            };
            if style.strikethrough {
                out.push(TextQuad::rect(
                    TextQuadKind::Shadow,
                    run.strikethrough_position() + offset,
                    run.strikethrough_size(),
                    color,
                ));
            }
            if style.underline {
                out.push(TextQuad::rect(
                    TextQuadKind::Shadow,
                    run.underline_position() + offset,
                    run.underline_size(),
                    color,
                ));
            }
        }
    }

    let mut current_section = u32::MAX;
    let mut color = LinearRgba::default();
    for glyph in &layout.glyphs {
        if glyph.section_index != current_section {
            current_section = glyph.section_index;
            color = style(current_section).map_or(LinearRgba::default(), |s| s.color.0.to_linear());
        }
        out.push(TextQuad {
            kind: TextQuadKind::Glyph,
            texture: Some(glyph.atlas_info.texture),
            rect: glyph.atlas_info.rect,
            center: glyph.position,
            size: glyph.atlas_info.rect.size(),
            // Color glyphs (emoji) carry their own color and must not be tinted.
            color: if glyph.atlas_info.is_alpha_mask {
                color
            } else {
                LinearRgba::WHITE
            },
            is_alpha_mask: glyph.atlas_info.is_alpha_mask,
        });
    }

    for run in &layout.run_geometry {
        let Some(style) = style(run.section_index) else {
            continue;
        };
        if style.strikethrough {
            out.push(TextQuad::rect(
                TextQuadKind::Decoration,
                run.strikethrough_position(),
                run.strikethrough_size(),
                style
                    .strikethrough_color
                    .map_or(style.color.0, |c| c.0)
                    .to_linear(),
            ));
        }
        if style.underline {
            out.push(TextQuad::rect(
                TextQuadKind::Decoration,
                run.underline_position(),
                run.underline_size(),
                style
                    .underline_color
                    .map_or(style.color.0, |c| c.0)
                    .to_linear(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GlyphAtlasInfo, PositionedGlyph, RunGeometry, TextEntity};
    use bevy_color::Color;
    use bevy_ecs::{system::SystemState, world::World};

    #[test]
    fn draw_order_and_colors() {
        let mut world = World::new();
        let section = world
            .spawn((
                TextColor(Color::linear_rgb(1., 0., 0.)),
                TextBackgroundColor(Color::linear_rgb(0., 0., 1.)),
                Underline,
            ))
            .id();

        let mut block = ComputedTextBlock::default();
        block.entities.push(TextEntity {
            entity: section,
            depth: 0,
            font_smoothing: Default::default(),
        });

        let glyph = |is_alpha_mask| PositionedGlyph {
            position: Vec2::new(5., 5.),
            atlas_info: GlyphAtlasInfo {
                texture: AssetId::default(),
                rect: Rect::new(0., 0., 10., 10.),
                offset: Vec2::ZERO,
                is_alpha_mask,
            },
            section_index: 0,
            line_index: 0,
        };
        let layout = TextLayoutInfo {
            glyphs: vec![glyph(true), glyph(false)],
            run_geometry: vec![RunGeometry {
                section_index: 0,
                bounds: Rect::new(0., 0., 20., 10.),
                strikethrough_y: 5.,
                strikethrough_thickness: 1.,
                underline_y: 9.,
                underline_thickness: 1.,
            }],
            ..Default::default()
        };

        let mut state = SystemState::<Query<TextSectionStyle>>::new(&mut world);
        let styles = state.get(&world).unwrap();
        let shadow = Some((Vec2::ONE, LinearRgba::BLACK));
        let mut out = Vec::new();
        text_quads(&mut out, &layout, &block, &styles, shadow);

        let kinds: Vec<_> = out.iter().map(|q| q.kind).collect();
        use TextQuadKind::*;
        assert_eq!(
            kinds,
            [Background, Shadow, Shadow, Shadow, Glyph, Glyph, Decoration]
        );
        assert_eq!(out[0].color, LinearRgba::BLUE);
        assert_eq!(out[1].center, Vec2::new(6., 6.));
        assert_eq!(out[4].color, LinearRgba::RED);
        assert_eq!(out[5].color, LinearRgba::WHITE);
        assert_eq!(out[6].color, LinearRgba::RED);
    }
}
