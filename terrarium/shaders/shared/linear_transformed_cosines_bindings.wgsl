@include linear_transformed_cosines.wgsl

@include material_pool.wgsl

const MAX_LTC_INSTANCES_PER_TILE: u32 = 128;

struct LtcConstants {
    instance_count: u32,
    range_bias: f32,
    _padding0: u32,
    _padding1: u32,
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
var<storage, read> ltc_instances: array<PackedLtcInstance>;

@group(5)
@binding(5)
var<storage, read> ltc_instances_inv_transform: array<mat3x4<f32>>;

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

fn LtcBindings::_evaluate(normal: vec3<f32>, view_dir: vec3<f32>, hit_point: vec3<f32>, _min_v: mat3x3<f32>, double_sided: bool, behind: bool,
    point0: vec3<f32>, point1: vec3<f32>, point2: vec3<f32>, point3: vec3<f32>) -> vec3<f32> {
    let t1: vec3<f32> = normalize(view_dir - normal * dot(view_dir, normal));
    let t2: vec3<f32> = cross(normal, t1);

    let min_v: mat3x3<f32> = _min_v * transpose(mat3x3<f32>(t1, t2, normal));

    let l0: vec3<f32> = normalize(min_v * (point0 - hit_point));
    let l1: vec3<f32> = normalize(min_v * (point1 - hit_point));
    let l2: vec3<f32> = normalize(min_v * (point2 - hit_point));
    let l3: vec3<f32> = normalize(min_v * (point3 - hit_point));

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

    return vec3<f32>(len * scale);
}

fn LtcBindings::shade(material: Material, instance_idx: u32, normal: vec3<f32>, view_dir: vec3<f32>, hit_point: vec3<f32>) -> vec3<f32> {
    let instance: LtcInstance = PackedLtcInstance::unpack(ltc_instances[instance_idx]);

    let point0 = LtcInstance::point0(instance);
    let point1 = LtcInstance::point1(instance);
    let point2 = LtcInstance::point2(instance);
    let point3 = LtcInstance::point3(instance);

    let dir: vec3<f32> = point0 - hit_point;
    let light_normal: vec3<f32> = cross(point1 - point0, point3 - point0);
    let behind: bool = dot(dir, light_normal) < 0.0;

    if (!instance.double_sided && !behind) {
        return vec3<f32>(0.0);
    }
    if (dot(normalize(point0 - hit_point), normal) < 0.0
        && dot(normalize(point1 - hit_point), normal) < 0.0
        && dot(normalize(point2 - hit_point), normal) < 0.0
        && dot(normalize(point3 - hit_point), normal) < 0.0
    ) {
        return vec3<f32>(0.0);
    }

    let n_dot_v: f32 = clamp(dot(normal, view_dir), 0.0, 1.0);
    let tex_coord = vec2<f32>(material.roughness, sqrt(1.0 - n_dot_v)) * LTC_LUT_SCALE + LTC_LUT_BIAS;

    let t1: vec4<f32> = textureSampleLevel(ltc_1, ltc_sampler, tex_coord, 0.0);
    let t2: vec4<f32> = textureSampleLevel(ltc_2, ltc_sampler, tex_coord, 0.0);

    let min_v = mat3x3<f32>(
        vec3<f32>(t1.x, 0.0, t1.y),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(t1.z, 0.0, t1.w)
    );

    var diffuse: vec3<f32> = LtcBindings::_evaluate(normal, view_dir, hit_point, IDENTITY_MAT3X3, instance.double_sided, behind,
        point0, point1, point2, point3);
    var specular: vec3<f32> = LtcBindings::_evaluate(normal, view_dir, hit_point, min_v, instance.double_sided, behind,
        point0, point1, point2, point3);

    let mspec = vec3<f32>(0.23);
    specular *= mspec * t2.x + (1.0 - mspec) * t2.y;

    let packed_inv_transform: mat3x4<f32> = ltc_instances_inv_transform[instance_idx];
    let inv_transform: mat4x4<f32> = mat4x4<f32>(
        vec4<f32>(packed_inv_transform[0].x, packed_inv_transform[1].x, packed_inv_transform[2].x, 0.0),
        vec4<f32>(packed_inv_transform[0].y, packed_inv_transform[1].y, packed_inv_transform[2].y, 0.0),
        vec4<f32>(packed_inv_transform[0].z, packed_inv_transform[1].z, packed_inv_transform[2].z, 0.0),
        vec4<f32>(packed_inv_transform[0].w, packed_inv_transform[1].w, packed_inv_transform[2].w, 1.0)
    );

    let area: f32 = LtcInstance::area(instance);
    let distance: f32 = LtcInstance::distance(instance, hit_point, inv_transform);

    let range_bias: f32 = ltc_constants.range_bias * instance.range_bias_factor;
    let attenuation: f32 = max(area / (distance * distance + area) - range_bias, 0.0);

    return attenuation * instance.color * (specular + material.color * diffuse);
}

fn LtcInstance::illuminated_aabb(_self: LtcInstance) -> Aabb {
    let area: f32 = LtcInstance::area(_self);
    let intensity: f32 = length(_self.color);

    let threshold: f32 = mix(0.01, 0.003, clamp(intensity / 3000.0, 0.0, 1.0));
    let range_bias: f32 = ltc_constants.range_bias * _self.range_bias_factor;
    let illumination_reach: f32 = sqrt(area * (1.0 - threshold - range_bias) / (threshold + range_bias));

    var p0: vec3<f32> = LtcInstance::point0(_self);
    var p1: vec3<f32> = LtcInstance::point1(_self);
    var p2: vec3<f32> = LtcInstance::point2(_self);
    var p3: vec3<f32> = LtcInstance::point3(_self);

    let right: vec3<f32> = normalize(_self.transform[0].xyz);
    let forward: vec3<f32> = normalize(_self.transform[2].xyz);
    p0 += (right + forward) * illumination_reach;
    p1 += (-right + forward) * illumination_reach;
    p2 += (right + -forward) * illumination_reach;
    p3 += (-right + -forward) * illumination_reach;

    let min_pos: vec3<f32> = min(min(p0, p1), min(p2, p3));
    let max_pos: vec3<f32> = max(max(p0, p1), max(p2, p3));

    let up: vec3<f32> = normalize(cross(p1 - p0, p3 - p0));
    let offset: vec3<f32> = up * illumination_reach;

    var illumination_min_pos: vec3<f32> = min(min_pos, min_pos + offset);
    var illumination_max_pos: vec3<f32> = max(max_pos, max_pos + offset);
    if (_self.double_sided) {
        illumination_min_pos = min(illumination_min_pos, min_pos - offset);
        illumination_max_pos = max(illumination_max_pos, max_pos - offset);
    }

    return Aabb::new(illumination_min_pos, illumination_max_pos);
}