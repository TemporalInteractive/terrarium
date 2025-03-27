struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

struct PushConstant {
    view_proj: mat4x4<f32>,
}

var<push_constant> pc : PushConstant;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let x: f32 = f32(i32(in_vertex_index) - 1);
    let y: f32 = f32(i32(in_vertex_index & 1u) * 2 - 1);
    let position = vec3<f32>(x, y, 0.0);

    var result: VertexOutput;
    result.position = pc.view_proj * vec4<f32>(position, 1.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

// @vertex
// fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
//     
//     let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
//     return vec4<f32>(x, y, 0.0, 1.0);
// }

// @fragment
// fn fs_main() -> @location(0) vec4<f32> {
//     return vec4<f32>(1.0, 0.0, 0.0, 1.0);
// }