@include terrarium/shaders/shared/packing.wgsl

struct PackedGBufferTexel {
    depth_ws: f32,
    normal_ws: PackedNormalizedXyz10,
    material_descriptor_idx: u32,
    tex_coord: u32,
}

struct GBufferTexel {
    depth_ws: f32,
    normal_ws: vec3<f32>,
    material_descriptor_idx: u32,
    tex_coord: vec2<f32>,
}

fn GBufferTexel::is_sky(_self: GBufferTexel) -> bool {
    return _self.depth_ws == 0.0;
}
 
fn PackedGBufferTexel::new(depth_ws: f32, normal_ws: vec3<f32>, material_descriptor_idx: u32, tex_coord: vec2<f32>) -> PackedGBufferTexel {
    return PackedGBufferTexel(
        depth_ws,
        PackedNormalizedXyz10::new(normal_ws, 0),
        material_descriptor_idx,
        pack2x16float(tex_coord)
    );
}

fn PackedGBufferTexel::unpack(_self: PackedGBufferTexel) -> GBufferTexel {
    return GBufferTexel(
        _self.depth_ws,
        PackedNormalizedXyz10::unpack(_self.normal_ws, 0),
        _self.material_descriptor_idx,
        unpack2x16float(_self.tex_coord)
    );
}