@include terrarium/shaders/shared/xr.wgsl

@include terrarium/shaders/shared/vertex_pool_bindings.wgsl
@include terrarium/shaders/shared/material_pool_bindings.wgsl

struct VertexOutput {
    @location(0) normal_ws: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @builtin(position) position_cs: vec4<f32>,
};

struct PushConstant {
    local_to_world_space: mat4x4<f32>,
    inv_trans_local_to_world_space: mat4x4<f32>,
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

    let normal: vec3<f32> = PackedNormalizedXyz10::unpack(packed_normal, 0);
    let tangent: vec3<f32> = PackedNormalizedXyz10::unpack(packed_tangent, 0);
    let bitangent: vec3<f32> = VertexPool::calculate_bitangent(normal, tangent, tangent_handiness);

    let normal_ws: vec3<f32> = (pc.inv_trans_local_to_world_space * vec4<f32>(normal, 0.0)).xyz;

    let position_world_space: vec4<f32> = pc.local_to_world_space * vec4<f32>(position, 1.0);
    let position_clip_space: vec4<f32> = xr_camera.world_to_clip_space[view_index] * position_world_space;

    var result: VertexOutput;
    result.normal_ws = normal_ws;
    result.tex_coord = tex_coord;
    result.position_cs = position_clip_space;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    // let tex_coord: vec2<f32> = MaterialPoolBindings::transform_uv(0, vertex.tex_coord);
    // let color: vec4<f32> = srgb_to_linear(_texture(0, tex_coord));

    let l: vec3<f32> = -normalize(vec3<f32>(0.1, -1.0, 0.2));
    let color: vec3<f32> = vec3<f32>(1.0, 0.8, 0.7) * max(dot(vertex.normal_ws, l), 0.2);

    // if (color.a < 0.5) {
    //     discard;
    // }

    return vec4<f32>(color, 1.0);
}