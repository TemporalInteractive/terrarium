@include shared/color.wgsl
@include shared/gbuffer.wgsl
@include shared/random.wgsl
@include shared/sampling.wgsl
@include shared/xr.wgsl

struct Constants {
    resolution: vec2<u32>,
    shadow_resolution: vec2<u32>,
    seed: u32,
    sample_count: u32,
    radius: f32,
    intensity: f32,
    bias: f32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<uniform> xr_camera: XrCamera;

@group(0)
@binding(2)
var<storage, read> gbuffer_left: array<PackedGBufferTexel>;
@group(0)
@binding(3)
var<storage, read> gbuffer_right: array<PackedGBufferTexel>;

@group(0)
@binding(4)
var shadow: texture_storage_2d_array<r16float, read_write>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let shadow_id: vec2<u32> = global_id.xy;
    if (any(shadow_id >= constants.shadow_resolution)) { return; }
    
    let id = vec2<u32>(
        u32(f32(constants.resolution.x) / f32(constants.shadow_resolution.x) * f32(shadow_id.x)),
        u32(f32(constants.resolution.y) / f32(constants.shadow_resolution.y) * f32(shadow_id.y))
    );
    let i: u32 = id.y * constants.resolution.x + id.x;

    var rng: u32 = pcg_hash(i ^ xor_shift_u32(constants.seed));

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        var gbuffer_texel: GBufferTexel;
        if (view_index == 0) {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_left[i]);
        } else {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_right[i]);
        }

        if (!GBufferTexel::is_sky(gbuffer_texel)) {
            let center_point_ws: vec3<f32> = gbuffer_texel.position_ws;

            let tangent_to_world = mat3x3<f32>(
                gbuffer_texel.tangent_ws,
                -GBufferTexel::bitangent_ws(gbuffer_texel),
                gbuffer_texel.normal_ws
            );

            var occlusion: f32 = 0.0;

            for (var j: u32 = 0; j < constants.sample_count; j += 1) {
                var scale: f32 = f32(j) / f32(constants.sample_count);
                scale = mix(0.1, 1.0, sqr(scale));
                let sample_point_ts: vec3<f32> = get_uniform_hemisphere_sample(random_uniform_float2(&rng)) * scale * constants.radius;
                let sample_point_ws: vec3<f32> = center_point_ws + tangent_to_world * sample_point_ts;

                var sample_uv: vec4<f32> = xr_camera.view_to_clip_space[view_index] * xr_camera.world_to_view_space[view_index] * vec4<f32>(sample_point_ws, 1.0);
                sample_uv = (sample_uv / sample_uv.w) * 0.5 + 0.5;
                sample_uv.y = 1.0 - sample_uv.y;

                let sample_id: vec2<u32> = vec2<u32>(sample_uv.xy * vec2<f32>(constants.resolution));
                let sample_i = sample_id.y * constants.resolution.x + sample_id.x;

                var sample_gbuffer_texel: GBufferTexel;
                if (view_index == 0) {
                    sample_gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_left[sample_i]);
                } else {
                    sample_gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_right[sample_i]);
                }

                if (!GBufferTexel::is_sky(sample_gbuffer_texel)) {
                    let range_check: f32 = smoothstep(1.0, 0.0, abs(gbuffer_texel.depth_ws - sample_gbuffer_texel.depth_ws));
                    let check: f32 = saturate(dot(sample_point_ws - center_point_ws, gbuffer_texel.normal_ws) * 10.0);

                    if (gbuffer_texel.depth_ws >= sample_gbuffer_texel.depth_ws + constants.bias) {
                        occlusion += check * range_check;
                    }
                }
            }

            let occlusion_factor: f32 = mix(1.0, 1.0 - (occlusion / f32(constants.sample_count)), constants.intensity);

            var shadow_factor: f32 = textureLoad(shadow, shadow_id, view_index).r;
            shadow_factor *= occlusion_factor;
            textureStore(shadow, shadow_id, view_index, vec4<f32>(vec3<f32>(shadow_factor), 1.0));
        }
    }
}