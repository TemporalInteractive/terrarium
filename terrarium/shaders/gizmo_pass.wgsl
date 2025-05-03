struct Constants {
    resolution: vec2<u32>,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

struct VertexOutput {
    @location(0) color: vec4<f32>,
    @builtin(position) position_cs: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @builtin(view_index) view_index: i32
) -> VertexOutput {


    var position_cs = position / vec2<f32>(f32(constants.resolution.x), f32(constants.resolution.y));
    position_cs = position_cs * 2.0 - 1.0;
    position_cs.y = -position_cs.y;

    var result: VertexOutput;
    result.color = color;
    result.position_cs = vec4<f32>(position_cs, 0.0, 1.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(vertex.color);
}