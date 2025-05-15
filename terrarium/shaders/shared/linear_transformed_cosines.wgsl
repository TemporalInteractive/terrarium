@include math.wgsl

struct LtcInstance {
    transform: mat4x4<f32>,
    inv_transform: mat4x4<f32>,
    color: vec3<f32>,
    double_sided: u32,
}

fn LtcInstance::point0(_self: LtcInstance) -> vec3<f32> {
    return (_self.transform * vec4<f32>(1.0, 0.0, 1.0, 1.0)).xyz;
}
fn LtcInstance::point1(_self: LtcInstance) -> vec3<f32> {
    return (_self.transform * vec4<f32>(-1.0, 0.0, 1.0, 1.0)).xyz;
}
fn LtcInstance::point2(_self: LtcInstance) -> vec3<f32> {
    return (_self.transform * vec4<f32>(-1.0, 0.0, -1.0, 1.0)).xyz;
}
fn LtcInstance::point3(_self: LtcInstance) -> vec3<f32> {
    return (_self.transform * vec4<f32>(1.0, 0.0, -1.0, 1.0)).xyz;
}

fn LtcInstance::area(_self: LtcInstance) -> f32 {
    let scale: vec3<f32> = get_mat4x4_scale(_self.transform);
    let width: f32 = scale.x * 2.0;
    let height: f32 = scale.z * 2.0;
    return width * height;
}

fn LtcInstance::distance(_self: LtcInstance, point: vec3<f32>) -> f32 {
    let point_local: vec3<f32> = (_self.inv_transform * vec4<f32>(point, 1.0)).xyz;

    let clamped_x: f32 = clamp(point_local.x, -1.0, 1.0);
    let clamped_z: f32 = clamp(point_local.z, -1.0, 1.0);
    let closest_point: vec3<f32> = vec3<f32>(clamped_x, 0.0, clamped_z);

    let scale: vec3<f32> = get_mat4x4_scale(_self.transform);

    return distance(point_local * scale, closest_point * scale); 
}

fn LtcInstance::illuminated_aabb(_self: LtcInstance) -> Aabb {
    let area: f32 = LtcInstance::area(_self);
    let illumination_reach: f32 = sqrt(area * ((1.0 - 0.01) / 0.01));

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
    if (_self.double_sided > 0) {
        illumination_min_pos = min(illumination_min_pos, min_pos - offset);
        illumination_max_pos = max(illumination_max_pos, max_pos - offset);
    }

    return Aabb::new(illumination_min_pos, illumination_max_pos);
}