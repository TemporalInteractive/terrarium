@include math.wgsl

struct LtcInstance {
    transform: mat4x4<f32>,
    color: vec3<f32>,
    double_sided: u32,
}

fn LtcInstance::point0(_self: LtcInstance, scale: f32) -> vec3<f32> {
    return (_self.transform * vec4<f32>(scale, 0.0, scale, 1.0)).xyz;
}
fn LtcInstance::point1(_self: LtcInstance, scale: f32) -> vec3<f32> {
    return (_self.transform * vec4<f32>(-scale, 0.0, scale, 1.0)).xyz;
}
fn LtcInstance::point2(_self: LtcInstance, scale: f32) -> vec3<f32> {
    return (_self.transform * vec4<f32>(-scale, 0.0, -scale, 1.0)).xyz;
}
fn LtcInstance::point3(_self: LtcInstance, scale: f32) -> vec3<f32> {
    return (_self.transform * vec4<f32>(scale, 0.0, -scale, 1.0)).xyz;
}

fn LtcInstance::illuminated_aabb(_self: LtcInstance) -> Aabb {
    let intensity: f32 = length(_self.color);
    let illumination_reach: f32 = intensity * 0.1;

    let p0: vec3<f32> = LtcInstance::point0(_self, illumination_reach * 10.0 + 1.0);
    let p1: vec3<f32> = LtcInstance::point1(_self, illumination_reach * 10.0 + 1.0);
    let p2: vec3<f32> = LtcInstance::point2(_self, illumination_reach * 10.0 + 1.0);
    let p3: vec3<f32> = LtcInstance::point3(_self, illumination_reach * 10.0 + 1.0);

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