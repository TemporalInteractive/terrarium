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

fn GBufferTexel::position_ws(_self: GBufferTexel, id: vec2<u32>, resolution: vec2<u32>, view_index: u32, xr_camera: XrCamera) -> vec3<f32> {
    let pixel_center = vec2<f32>(id) + xr_camera.jitter;
    var uv: vec2<f32> = (pixel_center / vec2<f32>(constants.resolution)) * 2.0 - 1.0;
    uv.y = -uv.y;
    let origin: vec3<f32> = (xr_camera.view_to_world_space[view_index] * vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz;
    let targt: vec4<f32> = xr_camera.clip_to_view_space[view_index] * vec4<f32>(uv, 1.0, 1.0);
    let direction: vec3<f32> = (xr_camera.view_to_world_space[view_index] * vec4<f32>(normalize(targt.xyz), 0.0)).xyz;

    return origin + direction * _self.depth_ws;
}
 
fn PackedGBufferTexel::new(depth_ws: f32, normal_ws: vec3<f32>, material_descriptor_idx: u32, tex_coord: vec2<f32>) -> PackedGBufferTexel {
    let fract_tex_coord: vec2<f32> = fract(tex_coord);

    return PackedGBufferTexel(
        depth_ws,
        PackedNormalizedXyz10::new(normal_ws, 0),
        material_descriptor_idx,
        pack2x16unorm(fract_tex_coord),
    );
}

fn PackedGBufferTexel::unpack(_self: PackedGBufferTexel) -> GBufferTexel {
    return GBufferTexel(
        _self.depth_ws,
        PackedNormalizedXyz10::unpack(_self.normal_ws, 0),
        _self.material_descriptor_idx,
        unpack2x16unorm(_self.tex_coord)
    );
}