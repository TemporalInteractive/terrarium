@include shared/math.wgsl

// Source: https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/mipmap/blit.wgsl

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let tc = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4<f32>(
        tc.x * 2.0 - 1.0,
        1.0 - tc.y * 2.0,
        0.0, 1.0
    );
    result.tex_coords = tc;
    return result;
}

@group(0)
@binding(0)
var r_color: texture_2d_array<f32>;
@group(0)
@binding(1)
var r_sampler: sampler;

struct Constants {
    view_index_override: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

@group(0)
@binding(2)
var<uniform> constants: Constants;

@fragment
fn fs_main(
    vertex: VertexOutput,
    @builtin(view_index) _view_index: i32
) -> @location(0) vec4<f32> {
    var view_index: u32 = u32(_view_index);
    if (constants.view_index_override != U32_MAX) {
        view_index = constants.view_index_override;
    }

    return textureSample(r_color, r_sampler, vertex.tex_coords, i32(view_index));
}