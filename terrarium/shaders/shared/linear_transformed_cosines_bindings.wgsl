@include linear_transformed_cosines.wgsl

@include material_pool.wgsl

const MAX_LTC_INSTANCES_PER_TILE: u32 = 320;

struct LtcConstants {
    instance_count: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

@group(5)
@binding(0)
var<uniform> ltc_constants: LtcConstants;

@group(5)
@binding(1)
var ltc_1: texture_2d<f32>;

@group(5)
@binding(2)
var ltc_2: texture_2d<f32>;

@group(5)
@binding(3)
var ltc_sampler: sampler;

@group(5)
@binding(4)
var<storage, read> ltc_instances: array<LtcInstance>;

const LTC_LUT_SIZE: f32 = 64.0;
const LTC_LUT_SCALE: f32 = (LTC_LUT_SIZE - 1.0) / LTC_LUT_SIZE;
const LTC_LUT_BIAS: f32 = 0.5 / LTC_LUT_SIZE;

fn LtcBindings::_integrate_edge(v1: vec3<f32>, v2: vec3<f32>) -> vec3<f32> {
    let x: f32 = dot(v1, v2);
    let y: f32 = abs(x);

    let a: f32 = 0.8543985 + (0.4965155 + 0.0145206 * y) * y;
    let b: f32 = 3.4175940 + (4.1616724 + y) * y;
    let v: f32 = a / b;

    let theta_sintheta: f32 = select(
        0.5 * (1.0 / sqrt(max(1.0 - x * x, 1e-7))) - v,
        v,
        x > 0.0
    );

    return cross(v1, v2) * theta_sintheta;
}

fn LtcBindings::_evaluate(normal: vec3<f32>, view_dir: vec3<f32>, hit_point: vec3<f32>, _min_v: mat3x3<f32>, double_sided: bool,
    point0: vec3<f32>, point1: vec3<f32>, point2: vec3<f32>, point3: vec3<f32>) -> vec3<f32> {
    let t1: vec3<f32> = normalize(view_dir - normal * dot(view_dir, normal));
    let t2: vec3<f32> = cross(normal, t1);

    let min_v: mat3x3<f32> = _min_v * transpose(mat3x3<f32>(t1, t2, normal));

    let l0: vec3<f32> = normalize(min_v * (point0 - hit_point));
    let l1: vec3<f32> = normalize(min_v * (point1 - hit_point));
    let l2: vec3<f32> = normalize(min_v * (point2 - hit_point));
    let l3: vec3<f32> = normalize(min_v * (point3 - hit_point));
    
    // Opt: can be done once, instead of for both evals
    let dir: vec3<f32> = point0 - hit_point;
    let light_normal: vec3<f32> = cross(point1 - point0, point3 - point0);
    let behind: bool = dot(dir, light_normal) < 0.0;

    var vsum = vec3<f32>(0.0);
    vsum += LtcBindings::_integrate_edge(l0, l1);
    vsum += LtcBindings::_integrate_edge(l1, l2);
    vsum += LtcBindings::_integrate_edge(l2, l3);
    vsum += LtcBindings::_integrate_edge(l3, l0);

    let len: f32 = length(vsum);

    var z: f32 = vsum.z / len;
    if (behind) {
        z = -z;
    }

    let uv = vec2<f32>(z * 0.5 + 0.5, len) * LTC_LUT_SCALE + LTC_LUT_BIAS;

    let scale: f32 = textureSampleLevel(ltc_2, ltc_sampler, uv, 0.0).w;

    var sum: f32 = len * scale;
    if (!behind && !double_sided) {
        sum = 0.0;
    }

    return vec3<f32>(sum);
}

fn LtcBindings::shade(material: Material, instance: LtcInstance, normal: vec3<f32>, view_dir: vec3<f32>, hit_point: vec3<f32>) -> vec3<f32> {
    let n_dot_v: f32 = clamp(dot(normal, view_dir), 0.0, 1.0);
    let tex_coord = vec2<f32>(material.roughness, sqrt(1.0 - n_dot_v)) * LTC_LUT_SCALE + LTC_LUT_BIAS;

    let t1: vec4<f32> = textureSampleLevel(ltc_1, ltc_sampler, tex_coord, 0.0);
    let t2: vec4<f32> = textureSampleLevel(ltc_2, ltc_sampler, tex_coord, 0.0);

    let min_v = mat3x3<f32>(
        vec3<f32>(t1.x, 0.0, t1.y),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(t1.z, 0.0, t1.w)
    );

    let double_sided: bool = instance.double_sided > 0;
    let point0 = LtcInstance::point0(instance, 1.0);
    let point1 = LtcInstance::point1(instance, 1.0);
    let point2 = LtcInstance::point2(instance, 1.0);
    let point3 = LtcInstance::point3(instance, 1.0);

    let diffuse: vec3<f32> = LtcBindings::_evaluate(normal, view_dir, hit_point, IDENTITY_MAT3X3, double_sided,
        point0, point1, point2, point3);
    var specular: vec3<f32> = LtcBindings::_evaluate(normal, view_dir, hit_point, min_v, double_sided,
        point0, point1, point2, point3);

    let mspec = vec3<f32>(0.23);
    specular *= mspec * t2.x + (1.0 - mspec) * t2.y;

    return instance.color * (specular + material.color * diffuse);
}