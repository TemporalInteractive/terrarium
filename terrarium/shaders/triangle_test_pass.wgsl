struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

struct PushConstant {
    view_proj: mat4x4<f32>,
}

var<push_constant> pc : PushConstant;

// @group(0)
// @binding(0)
// var<uniform> xr_view_proj: array<mat4x4<f32>, 2>;

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    //@builtin(view_index) view_index: i32
) -> VertexOutput {
    let x: f32 = f32(i32(in_vertex_index) - 1);
    let y: f32 = f32(i32(in_vertex_index & 1u) * 2 - 1);
    let position = vec3<f32>(x, y, -5.0);

    var result: VertexOutput;
    result.position = pc.view_proj * vec4<f32>(position, 1.0);
    // result.position = xr_view_proj[view_index] * vec4<f32>(position, 1.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}