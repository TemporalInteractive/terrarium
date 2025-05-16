@include math.wgsl

struct LtcInstance {
    transform: mat4x4<f32>,
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

fn LtcInstance::distance(_self: LtcInstance, point: vec3<f32>, inv_transform: mat4x4<f32>) -> f32 {
    let point_local: vec3<f32> = (inv_transform * vec4<f32>(point, 1.0)).xyz;

    let clamped_x: f32 = clamp(point_local.x, -1.0, 1.0);
    let clamped_z: f32 = clamp(point_local.z, -1.0, 1.0);
    let closest_point: vec3<f32> = vec3<f32>(clamped_x, 0.0, clamped_z);

    let scale: vec3<f32> = get_mat4x4_scale(_self.transform);

    return distance(point_local * scale, closest_point * scale); 
}