use crate::{ColorMaterial, MeshMaterial2d};
use bevy_asset::{AssetId, Assets, Handle, RenderAssetUsages};
use bevy_color::ColorToComponents;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    query::With,
    system::{Commands, Local, Query, ResMut},
};
use bevy_image::Image;
use bevy_math::Vec2;
use bevy_mesh::{Indices, Mesh, Mesh2d, PrimitiveTopology};
use bevy_platform::collections::HashMap;
use bevy_sprite::{Anchor, Text2dShadow};
use bevy_text::{
    text_quads, ComputedTextBlock, TextBounds, TextLayoutInfo, TextQuad, TextSectionStyle,
};
use bevy_transform::components::Transform;

// Marks the mesh children spawned under a `Text2d` entity, one per run of quads sharing a texture.
#[derive(Component)]
pub struct Text2dMesh;

// Quads emitted last time this text's meshes were rebuilt, so unchanged text is left alone.
#[derive(Component, Default)]
pub(crate) struct Text2dQuads(Vec<TextQuad>);

// Sibling batches share the parent's z; nudge each one so blending order matches draw order.
const BATCH_Z_STEP: f32 = 1e-4;

pub(crate) fn update_text2d_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    // ponytail: strong handles keyed by atlas texture, never evicted; atlases are few and long-lived.
    mut material_cache: Local<HashMap<Option<AssetId<Image>>, Handle<ColorMaterial>>>,
    mut quads: Local<Vec<TextQuad>>,
    mut texts: Query<(
        Entity,
        &ComputedTextBlock,
        &TextLayoutInfo,
        &TextBounds,
        &Anchor,
        Option<&Text2dShadow>,
        Option<&Children>,
        &mut Text2dQuads,
    )>,
    styles: Query<TextSectionStyle>,
    mut batches: Query<(&Mesh2d, &mut MeshMaterial2d<ColorMaterial>), With<Text2dMesh>>,
) {
    for (entity, block, layout, bounds, anchor, shadow, children, mut cache) in &mut texts {
        let scale_factor = layout.scale_factor;
        let inverse_scale_factor = scale_factor.recip();

        // `Text2dShadow::offset` is in logical y-up units; convert it into text space.
        let shadow = shadow.map(|shadow| {
            (
                shadow.offset * Vec2::new(scale_factor, -scale_factor),
                shadow.color.to_linear(),
            )
        });

        // ponytail: regenerates quads every frame and diffs; switch to change detection on the
        // section style components if this shows up in profiles.
        quads.clear();
        text_quads(&mut quads, layout, block, &styles, shadow);
        if *quads == cache.0 {
            continue;
        }
        cache.0.clone_from(&quads);

        let size = Vec2::new(
            bounds.width.unwrap_or(layout.size.x * inverse_scale_factor),
            bounds
                .height
                .unwrap_or(layout.size.y * inverse_scale_factor),
        );
        let top_left = (Anchor::TOP_LEFT.0 - anchor.as_vec()) * size;
        // Text space is y-down physical pixels; flip and scale it into the entity's local space.
        let to_local =
            |p: Vec2| top_left + p * Vec2::new(inverse_scale_factor, -inverse_scale_factor);

        let existing: Vec<Entity> = children
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .filter(|child| batches.contains(*child))
            .collect();
        let mut existing = existing.into_iter();

        let mut start = 0;
        let mut batch_index = 0;
        while start < quads.len() {
            let texture = quads[start].texture;
            let end = quads[start..]
                .iter()
                .position(|quad| quad.texture != texture)
                .map_or(quads.len(), |len| start + len);
            let texture_size = texture.and_then(|id| images.get(id)).map(Image::size_f32);
            let mesh = batch_mesh(&quads[start..end], texture_size, to_local);
            start = end;

            let material = material_cache
                .entry(texture)
                .or_insert_with(|| {
                    materials.add(ColorMaterial {
                        texture: texture.and_then(|id| images.get_strong_handle(id)),
                        ..Default::default()
                    })
                })
                .clone();

            match existing.next() {
                Some(child) => {
                    let (mesh2d, mut mesh_material) = batches.get_mut(child).unwrap();
                    if let Some(mut slot) = meshes.get_mut(mesh2d.id()) {
                        *slot = mesh;
                    }
                    if mesh_material.id() != material.id() {
                        mesh_material.0 = material;
                    }
                }
                None => {
                    commands.spawn((
                        Text2dMesh,
                        Mesh2d(meshes.add(mesh)),
                        MeshMaterial2d(material),
                        Transform::from_xyz(0., 0., batch_index as f32 * BATCH_Z_STEP),
                        ChildOf(entity),
                    ));
                }
            }
            batch_index += 1;
        }

        for child in existing {
            commands.entity(child).despawn();
        }
    }
}

fn batch_mesh(
    quads: &[TextQuad],
    texture_size: Option<Vec2>,
    to_local: impl Fn(Vec2) -> Vec2,
) -> Mesh {
    let uv_scale = texture_size.map_or(Vec2::ZERO, Vec2::recip);
    let mut positions = Vec::with_capacity(quads.len() * 4);
    let mut uvs = Vec::with_capacity(quads.len() * 4);
    let mut colors = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);

    for quad in quads {
        let base = positions.len() as u32;
        let half = quad.size / 2.;
        let (min, max) = (quad.center - half, quad.center + half);
        let (uv_min, uv_max) = (quad.rect.min * uv_scale, quad.rect.max * uv_scale);
        // Counterclockwise once flipped into y-up: top-left, bottom-left, bottom-right, top-right.
        for (position, uv) in [
            (min, uv_min),
            (Vec2::new(min.x, max.y), Vec2::new(uv_min.x, uv_max.y)),
            (max, uv_max),
            (Vec2::new(max.x, min.y), Vec2::new(uv_max.x, uv_min.y)),
        ] {
            positions.push(to_local(position).extend(0.).to_array());
            uvs.push(uv.to_array());
            colors.push(quad.color.to_f32_array());
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::{system::RunSystemOnce, world::World};
    use bevy_math::Rect;
    use bevy_text::{GlyphAtlasInfo, PositionedGlyph};

    fn glyph(texture: AssetId<Image>, x: f32) -> PositionedGlyph {
        PositionedGlyph {
            position: Vec2::new(x, 5.),
            atlas_info: GlyphAtlasInfo {
                texture,
                rect: Rect::new(0., 0., 10., 10.),
                offset: Vec2::ZERO,
                is_alpha_mask: true,
            },
            section_index: 0,
            line_index: 0,
        }
    }

    fn batch_vertex_counts(world: &World, text: Entity) -> Vec<usize> {
        let meshes = world.resource::<Assets<Mesh>>();
        world
            .get::<Children>(text)
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .map(|child| {
                let mesh = world.get::<Mesh2d>(child).unwrap();
                meshes.get(mesh.id()).unwrap().count_vertices()
            })
            .collect()
    }

    #[test]
    fn one_mesh_per_texture_run() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<ColorMaterial>>();
        world.init_resource::<Assets<Image>>();
        let a = world.resource_mut::<Assets<Image>>().add(Image::default());
        let b = world.resource_mut::<Assets<Image>>().add(Image::default());

        let layout = |glyphs| TextLayoutInfo {
            scale_factor: 1.,
            glyphs,
            ..Default::default()
        };
        let text = world
            .spawn((
                layout(vec![
                    glyph(a.id(), 5.),
                    glyph(a.id(), 15.),
                    glyph(b.id(), 25.),
                ]),
                ComputedTextBlock::default(),
                TextBounds::default(),
                Anchor::default(),
                Text2dQuads::default(),
            ))
            .id();

        world.run_system_once(update_text2d_meshes).unwrap();
        assert_eq!(batch_vertex_counts(&world, text), [8, 4]);
        let first = world.get::<Children>(text).unwrap()[0];

        // Unchanged text leaves its children alone.
        world.run_system_once(update_text2d_meshes).unwrap();
        assert_eq!(world.get::<Children>(text).unwrap()[0], first);

        // One texture now: the first child is reused, the extra one despawned.
        world.get_mut::<TextLayoutInfo>(text).unwrap().glyphs = vec![glyph(b.id(), 5.)];
        world.run_system_once(update_text2d_meshes).unwrap();
        assert_eq!(batch_vertex_counts(&world, text), [4]);
        assert_eq!(world.get::<Children>(text).unwrap()[0], first);
    }
}
