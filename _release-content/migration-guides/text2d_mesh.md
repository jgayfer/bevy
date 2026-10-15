---
title: "`Text2d` renders through the 2D mesh pipeline"
pull_requests: []
---

`Text2d` is now drawn with `Mesh2d` and `ColorMaterial`. Each `Text2d` entity gets child entities, marked with `Text2dMesh`, that hold one mesh per run of text quads sharing a font atlas texture. These children are managed by `bevy_sprite_render`; do not add your own components to them.

The remaining parts of the old instanced sprite pipeline, which only `Text2d` still used, have been removed from `bevy_sprite_render`: `SpritePipeline`, `SpritePipelineKey`, `ExtractedSprite`, `ExtractedSprites`, `ExtractedSpriteKind`, `ExtractedSlice`, `ExtractedSlices`, `SpriteAssetEvents`, `SpriteInstance`, `SpriteMeta`, `SpriteViewBindGroup`, `SpriteBatch`, `SpriteBatches`, `ImageBindGroups`, `DrawSprite` with its render commands, `init_sprite_pipeline`, `extract_sprite_events`, `extract_text2d_sprite`, `queue_sprites`, `prepare_sprite_view_bind_groups` and `prepare_sprite_image_bind_groups`. The `sprite.wesl` and `sprite_view_bindings.wesl` shaders are gone too.

Code that pushed into `ExtractedSprites` should instead spawn entities with `Mesh2d` and a `Material2d`, as `Sprite` already does via `SpriteMeshMaterial`.
