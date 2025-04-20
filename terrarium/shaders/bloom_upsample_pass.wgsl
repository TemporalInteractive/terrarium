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
        let a: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x, uv.y + y), view_index, 0.0).rgb;
        let b: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x, uv.y + y), view_index, 0.0).rgb;
        let c: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x, uv.y + y), view_index, 0.0).rgb;

        let d: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x, uv.y), view_index, 0.0).rgb;
        let e: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x, uv.y), view_index, 0.0).rgb;
        let f: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x, uv.y), view_index, 0.0).rgb;

        let g: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x - x, uv.y - y), view_index, 0.0).rgb;
        let h: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x, uv.y - y), view_index, 0.0).rgb;
        let i: vec3<f32> = textureSampleLevel(src, src_sampler, vec2<f32>(uv.x + x, uv.y - y), view_index, 0.0).rgb;

        var result: vec3<f32> = e * 4.0;
        result += (b + d + f + h) * 2.0;
        result += (a + c + g + i) * 0.0625;
        result /= 16.0;

        result = result * constants.intensity + textureLoad(dst, id, view_index).rgb;
        textureStore(dst, id, view_index, vec4<f32>(result, 1.0));
    }
}