@include packing.wgsl
@include xr.wgsl

struct PackedGBufferTexel {
    position_ws: vec3<f32>,
    depth_ws: f32,
    normal_ws: PackedNormalizedXyz10,
    tangent_ws: PackedNormalizedXyz10,
    material_descriptor_idx: u32,
    tex_coord: u32,
    velocity: vec2<f32>,
    _padding0: u32,
    _padding1: u32,
}

struct GBufferTexel {
    position_ws: vec3<f32>,
    depth_ws: f32,
    normal_ws: vec3<f32>,
    tangent_ws: vec3<f32>,
    material_descriptor_idx: u32,
    velocity: vec2<f32>,
    tex_coord: vec2<f32>,
}

fn GBufferTexel::is_sky(_self: GBufferTexel) -> bool {
    return _self.depth_ws == 0.0;
}

fn GBufferTexel::bitangent_ws(_self: GBufferTexel) -> vec3<f32> {
    return cross(_self.normal_ws, _self.tangent_ws);
}
 
fn PackedGBufferTexel::new(position_ws: vec3<f32>, depth_ws: f32, normal_ws: vec3<f32>, tangent_ws: vec3<f32>, material_descriptor_idx: u32, tex_coord: vec2<f32>, velocity: vec2<f32>) -> PackedGBufferTexel {
    let fract_tex_coord: vec2<f32> = fract(tex_coord);

    return PackedGBufferTexel(
        position_ws,
        depth_ws,
        PackedNormalizedXyz10::new(normal_ws, 0),
        PackedNormalizedXyz10::new(tangent_ws, 0),
        material_descriptor_idx,
        pack2x16unorm(fract_tex_coord),
        velocity,
        0, 0
    );
}

fn PackedGBufferTexel::unpack(_self: PackedGBufferTexel) -> GBufferTexel {
    return GBufferTexel(
        _self.position_ws,
        _self.depth_ws,
        PackedNormalizedXyz10::unpack(_self.normal_ws, 0),
        PackedNormalizedXyz10::unpack(_self.tangent_ws, 0),
        _self.material_descriptor_idx,
        _self.velocity,
        unpack2x16unorm(_self.tex_coord),
    );
}