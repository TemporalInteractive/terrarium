@include shared/xr.wgsl

struct Constants {
    resolution: vec2<u32>,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
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
    result.position += vec4<f32>(xr_camera.jitter / vec2<f32>(constants.resolution) * result.position.w, 0.0, 0.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(vertex.color.rgb, 1.0);
}