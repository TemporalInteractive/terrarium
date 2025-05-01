@include shared/color.wgsl

struct Constants {
    src_resolution: vec2<u32>,
    dst_resolution: vec2<u32>,
    radius: f32,
    intensity: f32,
    src_mip_level: u32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var src: texture_2d_array<f32>;

@group(0)
@binding(2)
var src_sampler: sampler;

@group(0)
@binding(3)
var dst: texture_storage_2d_array<rgba16float, read_write>;

fn karis_average(c: vec3<f32>) -> f32 {
    let luma: f32 = linear_to_luma(c) * 0.25;
    return 1.0 / (1.0 + luma);
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.dst_resolution)) { return; }

    let uv: vec2<f32> = (vec2<f32>(id) + vec2<f32>(0.5)) / vec2<f32>(constants.dst_resolution);
    let x: f32 = constants.radius / f32(constants.src_resolution.x);
    let y: f32 = constants.radius / f32(constants.src_resolution.y);

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        let a: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x * 2.0, uv.y + y * 2.0), view_index, 0.0).rgb;
        let b: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x, uv.y + y * 2.0), view_index, 0.0).rgb;
        let c: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x * 2.0, uv.y + y * 2.0), view_index, 0.0).rgb;

        let d: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x * 2.0, uv.y), view_index, 0.0).rgb;
        let e: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x, uv.y), view_index, 0.0).rgb;
        let f: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x * 2.0, uv.y), view_index, 0.0).rgb;

        let g: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x * 2.0, uv.y - y * 2.0), view_index, 0.0).rgb;
        let h: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x, uv.y - y * 2.0), view_index, 0.0).rgb;
        let i: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x * 2.0, uv.y - y * 2.0), view_index, 0.0).rgb;

        let j: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x, uv.y + y), view_index, 0.0).rgb;
        let k: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x, uv.y + y), view_index, 0.0).rgb;
        let l: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x, uv.y - y), view_index, 0.0).rgb;
        let m: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x, uv.y - y), view_index, 0.0).rgb;

        var result: vec3<f32>;
        if (constants.src_mip_level == 0) {
            var c0: vec3<f32> = (a + b + d + e) * (0.125 / 4.0);
            var c1: vec3<f32> = (b + c + e + f) * (0.125 / 4.0);
            var c2: vec3<f32> = (d + e + g + h) * (0.125 / 4.0);
            var c3: vec3<f32> = (e + f + h + i) * (0.125 / 4.0);
            var c4: vec3<f32> = (j + k + l + m) * (0.5 / 4.0);

            c0 *= karis_average(c0);
            c1 *= karis_average(c1);
            c2 *= karis_average(c2);
            c3 *= karis_average(c3);
            c4 *= karis_average(c4);

            result = c0 + c1 + c2 + c3 + c4;
            result = max(result, vec3<f32>(0.0001));
        } else {
            result = e * 0.125;
            result += (a + c + g + i) * 0.03125;
            result += (b + d + f + h) * 0.0625;
            result += (j + k + l + m) * 0.125;
        }

        textureStore(dst, id, view_index, vec4<f32>(result, 1.0));
    }
}