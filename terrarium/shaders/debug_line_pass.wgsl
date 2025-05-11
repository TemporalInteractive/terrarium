@include shared/xr.wgsl

@group(0)
@binding(0)
var<uniform> xr_camera: XrCamera;

struct VertexOutput {
    @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec4<f32>,
    @location(1) color: vec4<f32>,
    @builtin(view_index) view_index: i32
) -> VertexOutput {
    var result: VertexOutput;
    result.color = color;
    result.position = xr_camera.view_to_clip_space[view_index] * xr_camera.world_to_view_space[view_index] * vec4<f32>(position.xyz, 1.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(vertex.color.rgb, 1.0);
}