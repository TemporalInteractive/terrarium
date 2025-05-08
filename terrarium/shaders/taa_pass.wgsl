@include shared/color.wgsl
@include shared/gbuffer.wgsl
@include shared/math.wgsl

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
var color: texture_storage_2d_array<rgba16float, read_write>;

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

@group(0)
@binding(6)
var<uniform> xr_camera: XrCamera;

fn cubicHermite(A: vec3<f32>, B: vec3<f32>, C: vec3<f32>, D: vec3<f32>, t: f32) -> vec3<f32> {
    let t2: f32 = t * t;
    let t3: f32 = t2 * t;
    let a: vec3<f32> = -A / 2.0 + (3.0 * B) / 2.0 - (3.0 * C) / 2.0 + D / 2.0;
    let b: vec3<f32> = A - (5.0 * B) / 2.0 + 2.0 * C - D / 2.0;
    let c: vec3<f32> = -A / 2.0 + C / 2.0;
    let d: vec3<f32> = B;

    return a * t3 + b * t2 + c * t + d;
}

fn mitchellNetravali(x: f32) -> f32 {
    const B: f32 = 1.0 / 3.0;
    const C: f32 = 1.0 / 3.0;

    let ax: f32 = abs(x);
    if (ax < 1.0) {
        return ((12.0 - 9.0 * B - 6.0 * C) * ax * ax * ax + (-18.0 + 12.0 * B + 6.0 * C) * ax * ax +
                (6.0 - 2.0 * B)) / 6.0;
    } else if ((ax >= 1.0) && (ax < 2.0)) {
        return ((-B - 6.0 * C) * ax * ax * ax + (6.0 * B + 30.0 * C) * ax * ax + (-12.0 * B - 48.0 * C) * ax +
                (8.0 * B + 24.0 * C)) / 6.0;
    } else {
        return 0.0;
    }
}

fn fetchHistoryPixel(id: vec2<i32>, view_index: u32) -> vec3<f32> {
    if(any(id < vec2<i32>(0)) || any(id >= vec2<i32>(constants.resolution))) {
        return vec3<f32>(0.0);
    }

    return max(textureLoad(prev_color, id, view_index, 0).rgb, vec3<f32>(0.0));
}

fn bicubicHermiteHistorySample(uv: vec2<f32>, view_index: u32) -> vec3<f32> {
    let pixel: vec2<f32> = uv * vec2<f32>(constants.resolution) + vec2<f32>(0.5);
    let px_frac: vec2<f32> = fract(pixel);

    let ipixel: vec2<i32> = vec2<i32>(i32(pixel.x), i32(pixel.y)) - 1;

    let c00: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(-1, -1), view_index);
    let c10: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(0, -1), view_index);
    let c20: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(1, -1), view_index);
    let c30: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(2, -1), view_index);

    let c01: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(-1, 0), view_index);
    let c11: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(0, 0), view_index);
    let c21: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(1, 0), view_index);
    let c31: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(2, 0), view_index);

    let c02: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(-1, 1), view_index);
    let c12: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(0, 1), view_index);
    let c22: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(1, 1), view_index);
    let c32: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(2, 1), view_index);

    let c03: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(-1, 2), view_index);
    let c13: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(0, 2), view_index);
    let c23: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(1, 2), view_index);
    let c33: vec3<f32> = fetchHistoryPixel(ipixel + vec2<i32>(2, 2), view_index);

    let cp0x: vec3<f32> = cubicHermite(c00, c10, c20, c30, px_frac.x);
    let cp1x: vec3<f32> = cubicHermite(c01, c11, c21, c31, px_frac.x);
    let cp2x: vec3<f32> = cubicHermite(c02, c12, c22, c32, px_frac.x);
    let cp3x: vec3<f32> = cubicHermite(c03, c13, c23, c33, px_frac.x);

    return cubicHermite(cp0x, cp1x, cp2x, cp3x, px_frac.y);
}

const SQUARE_GROUP_SIDE: u32 = 8;
const NUM_THREADS_IN_GROUP: u32 = SQUARE_GROUP_SIDE * SQUARE_GROUP_SIDE;
const BORDER_SIZE: i32 = 1;
const SQUARE_GROUPSHARED_SIDE: u32 = SQUARE_GROUP_SIDE + 2 * u32(BORDER_SIZE);

var<workgroup> gs_inputs: array<array<vec3<f32>, SQUARE_GROUPSHARED_SIDE>, SQUARE_GROUPSHARED_SIDE>;

fn load_inputs(local_thread_index: u32, group_id: vec2<u32>, view_index: u32) {
    let group_top_left_corner: vec2<i32> = vec2<i32>(group_id * SQUARE_GROUP_SIDE) - vec2<i32>(BORDER_SIZE);

    if (view_index > 0) {
        workgroupBarrier();
    }

    for (var i: u32 = local_thread_index; i < SQUARE_GROUPSHARED_SIDE * SQUARE_GROUPSHARED_SIDE; i += NUM_THREADS_IN_GROUP) {
        let group_thread_id = vec2<u32>(i % SQUARE_GROUPSHARED_SIDE, i / SQUARE_GROUPSHARED_SIDE);
        let dispatch_thread_id: vec2<i32> = group_top_left_corner + vec2<i32>(group_thread_id);
        let input: vec3<f32> = linear_to_ycbcr(max(textureLoad(color, dispatch_thread_id, view_index).rgb, vec3<f32>(0.0)));
        gs_inputs[group_thread_id.x][group_thread_id.y] = input;
    }

    workgroupBarrier();
}

fn loaded_input(id: vec2<u32>) -> vec3<f32> {
    return gs_inputs[id.x][id.y];
}

fn fetchCenterFiltered(group_thread_id: vec2<u32>) -> vec3<f32> {
    var result = vec4<f32>(0.0);

    for (var y: i32 = -BORDER_SIZE; y <= BORDER_SIZE; y += 1) {
        for (var x: i32 = -BORDER_SIZE; x <= BORDER_SIZE; x += 1) {
            let neigh_pixel = vec2<u32>(vec2<i32>(group_thread_id) + vec2<i32>(x, y) + BORDER_SIZE);
            let neigh = vec4<f32>(loaded_input(neigh_pixel), 1.0);
            let dist: f32 = length(-xr_camera.jitter - vec2<f32>(f32(x), f32(y)));
            let weight: f32 = mitchellNetravali(dist);

            result += neigh * weight;
        }
    }

    return result.rgb / result.a;
}

@compute
@workgroup_size(SQUARE_GROUP_SIDE, SQUARE_GROUP_SIDE)
fn main(@builtin(global_invocation_id) global_thread_id: vec3<u32>,
    @builtin(local_invocation_index) local_thread_index: u32,
    @builtin(workgroup_id) group_id: vec3<u32>) {
    let id: vec2<u32> = global_thread_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let i: u32 = id.y * constants.resolution.x + id.x;
    let uv: vec2<f32> = (vec2<f32>(id) + 0.5) / vec2<f32>(constants.resolution);

    let group_thread_id = vec2<u32>(local_thread_index % SQUARE_GROUP_SIDE, local_thread_index / SQUARE_GROUP_SIDE);

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        load_inputs(local_thread_index, group_id.xy, view_index);

        var center: vec3<f32> = loaded_input(group_thread_id + vec2<u32>(BORDER_SIZE));

        var gbuffer_texel: GBufferTexel;
        if (view_index == 0) {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_left[i]);
        } else {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_right[i]);
        }

        if (GBufferTexel::is_sky(gbuffer_texel)) {
            continue;
        }

        var history_uv: vec2<f32> = uv - gbuffer_texel.velocity;
        //let history: vec3<f32> = linear_to_ycbcr(bicubicHermiteHistorySample(history_uv, view_index));

        let history_g: f32 = bicubicHermiteHistorySample(history_uv, view_index).g;
        var history: vec3<f32> = max(textureSampleLevel(prev_color, color_sampler, history_uv, view_index, 0.0).rgb, vec3<f32>(0.0));
        if (history.g > 1e-5) {
            history *= history_g / history.g;
        }
        history = linear_to_ycbcr(history);

        var mean = vec3<f32>(0.0);
        var variance = vec3<f32>(0.0);
        var accum_weights: f32 = 0.0;

        for (var y: i32 = -BORDER_SIZE; y <= BORDER_SIZE; y += 1) {
            for (var x: i32 = -BORDER_SIZE; x <= BORDER_SIZE; x += 1) {
                let neigh_pixel = vec2<u32>(vec2<i32>(group_thread_id) + vec2<i32>(x, y) + BORDER_SIZE);
                let neigh: vec3<f32> = loaded_input(neigh_pixel);

                let w: f32 = exp(-3.0 * f32(x * x + y * y) / 4.0);
                mean += neigh * w;
                variance += neigh * neigh * w;
                accum_weights += w;
            }
        }

        let ex: vec3<f32> = mean / accum_weights;
        let ex2: vec3<f32> = variance / accum_weights;
        let std_dev: vec3<f32> = sqrt(max(vec3<f32>(0.0), ex2 - ex * ex));

        let local_contrast: f32 = std_dev.x / (ex.x + 1e-5);

        let history_ss_coords: vec2<f32> = history_uv * vec2<f32>(constants.resolution);
        let texel_center_distance: f32 = dot(vec2<f32>(1.0), abs(0.5 - fract(history_ss_coords)));

        var box_size: f32 = 1.0;
        box_size *= mix(0.5, 1.0, smoothstep(-0.1, 0.3, local_contrast));
        box_size *= mix(0.5, 1.0, clamp(1.0 - texel_center_distance, 0.0, 1.0));

        let filtered_unjittered_center: vec3<f32> = fetchCenterFiltered(group_thread_id);

        const N_DEVIATIONS: f32 = 1.5;
        let nmin: vec3<f32> = mix(filtered_unjittered_center, ex, sqr(box_size)) - std_dev * box_size * N_DEVIATIONS;
        let nmax: vec3<f32> = mix(filtered_unjittered_center, ex, sqr(box_size)) + std_dev * box_size * N_DEVIATIONS;

        let valid_reprojection: bool = all(history_uv >= vec2<f32>(0.0)) && all(history_uv <= vec2<f32>(1.0));

        let clamped_history: vec3<f32> = clamp(history, nmin, nmax);
        let blend_factor: f32 = mix(1.0, 1.0 / 16.0, f32(valid_reprojection));

        let result: vec3<f32> = mix(clamped_history, filtered_unjittered_center, blend_factor);

        textureStore(color, id, view_index, vec4<f32>(ycbcr_to_linear(result), 1.0));
    }
}