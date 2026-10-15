use crate::{
    ExtractedSlice, ExtractedSlices, ExtractedSprite, ExtractedSpriteKind, ExtractedSprites,
};
use bevy_asset::AssetId;
use bevy_camera::visibility::ViewVisibility;
use bevy_ecs::{
    entity::Entity,
    system::{Commands, Local, Query, ResMut},
};
use bevy_math::{Vec2, Vec3};
use bevy_render::sync_world::TemporaryRenderEntity;
use bevy_render::Extract;
use bevy_sprite::{Anchor, Text2dShadow};
use bevy_text::{
    text_quads, ComputedTextBlock, TextBounds, TextLayoutInfo, TextQuad, TextSectionStyle,
};
use bevy_transform::prelude::GlobalTransform;

pub fn extract_text2d_sprite(
    mut commands: Commands,
    mut extracted_sprites: ResMut<ExtractedSprites>,
    mut extracted_slices: ResMut<ExtractedSlices>,
    mut quads: Local<Vec<TextQuad>>,
    text2d_query: Extract<
        Query<(
            Entity,
            &ViewVisibility,
            &ComputedTextBlock,
            &TextLayoutInfo,
            &TextBounds,
            &Anchor,
            Option<&Text2dShadow>,
            &GlobalTransform,
        )>,
    >,
    styles: Extract<Query<TextSectionStyle>>,
) {
    extracted_sprites.sprites.clear();
    extracted_slices.slices.clear();

    for (main_entity, view_visibility, block, layout, bounds, anchor, shadow, global_transform) in
        text2d_query.iter()
    {
        if !view_visibility.get() {
            continue;
        }

        let scale_factor = layout.scale_factor;
        let inverse_scale_factor = scale_factor.recip();
        let size = Vec2::new(
            bounds.width.unwrap_or(layout.size.x * inverse_scale_factor),
            bounds
                .height
                .unwrap_or(layout.size.y * inverse_scale_factor),
        );
        let top_left = (Anchor::TOP_LEFT.0 - anchor.as_vec()) * size;

        // Text space is y-down physical pixels; flip and scale it into the entity's local space.
        let transform = *global_transform
            * GlobalTransform::from_translation(top_left.extend(0.))
            * GlobalTransform::from_scale(Vec3::new(
                inverse_scale_factor,
                -inverse_scale_factor,
                1.,
            ));

        // `Text2dShadow::offset` is in logical y-up units; convert it into text space.
        let shadow = shadow.map(|shadow| {
            (
                shadow.offset * Vec2::new(scale_factor, -scale_factor),
                shadow.color.to_linear(),
            )
        });

        quads.clear();
        text_quads(&mut quads, layout, block, &styles, shadow);

        let mut i = 0;
        while i < quads.len() {
            let quad = quads[i];

            let Some(texture) = quad.texture else {
                extracted_sprites.sprites.push(ExtractedSprite {
                    main_entity,
                    render_entity: commands.spawn(TemporaryRenderEntity::default()).id(),
                    transform: transform
                        * GlobalTransform::from_translation(quad.center.extend(0.)),
                    color: quad.color,
                    image_handle_id: AssetId::default(),
                    flip_y: false,
                    kind: ExtractedSpriteKind::Single {
                        custom_size: quad.size,
                    },
                });
                i += 1;
                continue;
            };

            // A sprite instance has one color and one texture, so batch glyphs on both.
            let start = extracted_slices.slices.len();
            while let Some(next) = quads.get(i)
                && next.texture == Some(texture)
                && next.color == quad.color
            {
                extracted_slices.slices.push(ExtractedSlice {
                    offset: next.center,
                    rect: next.rect,
                    size: next.size,
                });
                i += 1;
            }
            extracted_sprites.sprites.push(ExtractedSprite {
                main_entity,
                render_entity: commands.spawn(TemporaryRenderEntity::default()).id(),
                transform,
                color: quad.color,
                image_handle_id: texture,
                flip_y: true,
                kind: ExtractedSpriteKind::Slices {
                    indices: start..extracted_slices.slices.len(),
                },
            });
        }
    }
}
