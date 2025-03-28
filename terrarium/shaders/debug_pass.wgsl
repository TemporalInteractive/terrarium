@include terrarium/shaders/packing.wgsl

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

// struct PushConstant {
//     view_proj: mat4x4<f32>,
// }

// var<push_constant> pc : PushConstant;

@group(0)
@binding(0)
var<uniform> xr_view_proj: array<mat4x4<f32>, 2>;

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

    // let x: f32 = f32(i32(in_vertex_index) - 1);
    // let y: f32 = f32(i32(in_vertex_index & 1u) * 2 - 1);
    // let position = vec3<f32>(x, y, -5.0);// * f32(view_index));

    var result: VertexOutput;
    //result.position = pc.view_proj * vec4<f32>(position + vec3<f32>(0.0, 0.0, 10.0), 1.0);
    result.position = xr_view_proj[view_index] * vec4<f32>(position + vec3<f32>(0.0, 0.0, 10.0), 1.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}