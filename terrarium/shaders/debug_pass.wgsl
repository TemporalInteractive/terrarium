@include terrarium/shaders/shared/packing.wgsl
@include terrarium/shaders/shared/xr.wgsl

struct VertexOutput {
    @location(0) normal_ws: vec3<f32>,
    @builtin(position) position_cs: vec4<f32>,
};

struct PushConstant {
    local_to_world_space: mat4x4<f32>,
}

var<push_constant> pc : PushConstant;

@group(0)
@binding(0)
var<uniform> xr_camera: XrCamera;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) _packed_normal: u32,
    @location(2) tex_coord: vec2<f32>,
    @location(3) _packed_tangent: u32,
    @location(4) tangent_handiness: f32,
    @builtin(view_index) view_index: i32
) -> VertexOutput {
    let packed_normal = PackedNormalizedXyz10(_packed_normal);
    let packed_tangent = PackedNormalizedXyz10(_packed_tangent);

    let position_world_space: vec4<f32> = pc.local_to_world_space * vec4<f32>(position, 1.0);
    let position_clip_space: vec4<f32> = xr_camera.view_to_clip_space[view_index] * xr_camera.world_to_view_space[view_index] * position_world_space;

    var result: VertexOutput;
    result.normal_ws = PackedNormalizedXyz10::unpack(packed_normal, 0);
    result.position_cs = position_clip_space;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(vertex.normal_ws * 0.5 + 0.5, 1.0);
}