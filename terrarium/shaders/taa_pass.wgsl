@include terrarium/shaders/shared/color.wgsl
@include terrarium/shaders/shared/gbuffer.wgsl
@include terrarium/shaders/shared/math.wgsl

struct Constants {
    resolution: vec2<u32>,
    history_influence: f32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var color: texture_storage_2d_array<rgba8unorm, read_write>;

@group(0)
@binding(2)
var prev_color: texture_2d_array<f32>;
@group(0)
@binding(3)
var color_sampler: sampler;

@group(0)
@binding(4)
var<storage, read> gbuffer_left: array<PackedGBufferTexel>;
@group(0)
@binding(5)
var<storage, read> gbuffer_right: array<PackedGBufferTexel>;

// Source: M. Pharr, W. Jakob, and G. Humphreys, Physically Based Rendering, Morgan Kaufmann, 2016.
fn mitchell_1d(_x: f32, B: f32, C: f32) -> f32 {
    let x: f32 = abs(2.0 * _x);
    let one_div_six: f32 = 1.0 / 6.0;

    if (x > 1) {
        return ((-B - 6.0 * C) * x * x * x + (6.0 * B + 30.0 * C) * x * x +
                (-12.0 * B - 48.0 * C) * x + (8.0 * B + 24.0 * C)) * one_div_six;
    } else {
        return ((12.0 - 9.0 * B - 6.0 * C) * x * x * x +
                (-18.0 + 12.0 * B + 6.0 * C) * x * x +
                (6.0 - 2.0 * B)) * one_div_six;
    }
}

// Source: https://github.com/playdeadgames/temporal
fn clip_aabb(aabb_min: vec3<f32>, aabb_max: vec3<f32>, hist_sample: vec3<f32>) -> vec3<f32> {
    let center: vec3<f32> = 0.5 * (aabb_max + aabb_min);
    let extents: vec3<f32> = 0.5 * (aabb_max - aabb_min);

    let ray_to_center: vec3<f32> = hist_sample - center;
    var ray_to_center_unit: vec3<f32> = ray_to_center.xyz / extents;
    ray_to_center_unit = abs(ray_to_center_unit);
    let ray_to_center_unit_max: f32 = max(ray_to_center_unit.x, max(ray_to_center_unit.y, ray_to_center_unit.z));

    if (ray_to_center_unit_max > 1.0) {
        return center + ray_to_center / ray_to_center_unit_max;
    } else {
        return hist_sample;
    }
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let i: u32 = id.y * constants.resolution.x + id.x;

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        var current: vec3<f32> = textureLoad(color, id, view_index).rgb;

        var weight_sum: f32 = sqr(mitchell_1d(0, 0.33, 0.33));
        var reconstructed: vec3<f32> = current * weight_sum;
        var first_moment: vec3<f32> = current;
        var second_moment: vec3<f32>  = current * current;

        var sample_count: f32 = 1.0;

        for (var x: i32 = -1; x <= 1; x += 1) {
            for (var y: i32 = -1; y <= 1; y += 1) {
                if (x == 0 && y == 0) {
                    continue;
                }

                let sample_pixel: vec2<i32> = vec2<i32>(id) + vec2<i32>(x, y);
                if (any(sample_pixel < vec2<i32>(0)) || any(sample_pixel >= vec2<i32>(constants.resolution))) {
                    continue;
                }

                let sample_color: vec3<f32> = max(textureLoad(color, sample_pixel, view_index).rgb, vec3<f32>(0.0)); // TODO: clamp required?
                var weight: f32 = mitchell_1d(f32(x), 0.33, 0.33) * mitchell_1d(f32(y), 0.33, 0.33);
                weight *= 1.0 / (1.0 + linear_to_luma(sample_color));

                reconstructed += sample_color * weight;
                weight_sum += weight;

                first_moment += sample_color;
                second_moment += sample_color * sample_color;

                sample_count += 1.0;
            }
        }

        reconstructed /= max(weight_sum, 1e-5);

        var gbuffer_texel: GBufferTexel;
        if (view_index == 0) {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_left[i]);
        } else {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_right[i]);
        }

        if (!GBufferTexel::is_sky(gbuffer_texel)) {
            var uv: vec2<f32> = (vec2<f32>(id) + vec2<f32>(0.5)) / vec2<f32>(constants.resolution);
            uv -= gbuffer_texel.velocity;

            let history: vec3<f32> = textureSampleLevel(prev_color, color_sampler, uv, view_index, 0.0).rgb;
            
            let mean: vec3<f32> = first_moment / sample_count;
            var stdev: vec3<f32> = abs(second_moment - (first_moment * first_moment) / sample_count);
            stdev /= (sample_count - 1.0);
            stdev = sqrt(stdev);

            let clipped_history: vec3<f32> = clip_aabb(mean - stdev, mean + stdev, history);

            let blend_weight: f32 = 1.0 - constants.history_influence;
            let current_weight: f32 = saturate(blend_weight * (1.0 / (1.0 + linear_to_luma(reconstructed))));
            let history_weight: f32 = saturate((1.0 - blend_weight) * (1.0 / (1.0 + linear_to_luma(clipped_history))));
            reconstructed = (current_weight * reconstructed + history_weight * clipped_history) / (current_weight + history_weight);
        }

        textureStore(color, id, view_index, vec4<f32>(reconstructed, 1.0));
    }
}