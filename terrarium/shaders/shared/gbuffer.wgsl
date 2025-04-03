@include terrarium/shaders/shared/packing.wgsl
@include terrarium/shaders/shared/xr.wgsl

struct PackedGBufferTexel {
    depth_ws: f32,
    normal_ws: PackedNormalizedXyz10,
    material_descriptor_idx: u32,
    tex_coord: u32,
    velocity: vec2<f32>,
    _padding0: u32,
    _padding1: u32,
}

struct GBufferTexel {
    depth_ws: f32,
    normal_ws: vec3<f32>,
    material_descriptor_idx: u32,
    tex_coord: vec2<f32>,
    velocity: vec2<f32>,
}

fn GBufferTexel::is_sky(_self: GBufferTexel) -> bool {
    return _self.depth_ws == 0.0;
}

fn GBufferTexel::position_ws(_self: GBufferTexel, id: vec2<u32>, resolution: vec2<u32>, view_index: u32, xr_camera: XrCamera) -> vec3<f32> {
    let ray: XrCameraRay = XrCamera::raygen(xr_camera, id, resolution, view_index);
    return ray.origin + ray.direction * _self.depth_ws;
}
 
fn PackedGBufferTexel::new(depth_ws: f32, normal_ws: vec3<f32>, material_descriptor_idx: u32, tex_coord: vec2<f32>, velocity: vec2<f32>) -> PackedGBufferTexel {
    let fract_tex_coord: vec2<f32> = fract(tex_coord);

    return PackedGBufferTexel(
        depth_ws,
        PackedNormalizedXyz10::new(normal_ws, 0),
        material_descriptor_idx,
        pack2x16unorm(fract_tex_coord),
        velocity,
        0, 0,
    );
}

fn PackedGBufferTexel::unpack(_self: PackedGBufferTexel) -> GBufferTexel {
    return GBufferTexel(
        _self.depth_ws,
        PackedNormalizedXyz10::unpack(_self.normal_ws, 0),
        _self.material_descriptor_idx,
        unpack2x16unorm(_self.tex_coord),
        _self.velocity,
    );
}