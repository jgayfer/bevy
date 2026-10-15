#![expect(missing_docs, reason = "Not all docs are written yet, see #3492.")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![doc(
    html_logo_url = "https://bevy.org/assets/icon.png",
    html_favicon_url = "https://bevy.org/assets/icon.png"
)]

//! Provides 2D sprite rendering functionality.

extern crate alloc;

mod mesh2d;
mod sprite_mesh;
#[cfg(feature = "bevy_text")]
mod text2d;
mod tilemap_chunk;

/// The sprite prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        ColorMaterial, ExtendedMaterial2d, MaterialExtension2d, MeshMaterial2d, SpriteMaterial,
        SpriteMaterialPlugin, SpriteMeshMaterial,
    };
}

pub use mesh2d::*;
pub use sprite_mesh::*;
pub use tilemap_chunk::*;

use bevy_app::prelude::*;
#[cfg(feature = "bevy_text")]
use bevy_asset::AssetEventSystems;
use bevy_core_pipeline::core_2d::{AlphaMask2d, Opaque2d};
use bevy_ecs::prelude::*;
use bevy_image::TextureAtlasPlugin;
use bevy_render::{
    batching::sort_binned_render_phase, sync_world::SyncToRenderWorld, Render, RenderApp,
    RenderSystems,
};
use bevy_sprite::Sprite;

#[cfg(feature = "bevy_text")]
pub use crate::text2d::Text2dMesh;

/// Adds support for 2D sprite rendering.
#[derive(Default)]
pub struct SpriteRenderPlugin;

impl Plugin for SpriteRenderPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TextureAtlasPlugin>() {
            app.add_plugins(TextureAtlasPlugin);
        }

        app.add_plugins((
            Mesh2dRenderPlugin,
            Materials2dPlugin,
            ColorMaterialPlugin,
            SpriteMeshPlugin,
            TilemapChunkPlugin,
            TilemapChunkMaterialPlugin,
        ));
        app.register_required_components::<Sprite, SyncToRenderWorld>();

        #[cfg(feature = "bevy_text")]
        app.register_required_components::<bevy_sprite::Text2d, text2d::Text2dQuads>()
            .add_systems(
                PostUpdate,
                text2d::update_text2d_meshes
                    .in_set(bevy_sprite::Text2dSystems)
                    .after(bevy_sprite::update_text2d_layout)
                    .before(check_entities_needing_specialization::<ColorMaterial>)
                    .before(mark_2d_meshes_as_changed_if_their_materials_changed::<ColorMaterial>)
                    .before(bevy_mesh::mark_2d_meshes_as_changed_if_their_assets_changed)
                    .before(AssetEventSystems),
            );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                (
                    sort_binned_render_phase::<Opaque2d>.in_set(RenderSystems::PhaseSort),
                    sort_binned_render_phase::<AlphaMask2d>.in_set(RenderSystems::PhaseSort),
                ),
            );
        };
    }
}
