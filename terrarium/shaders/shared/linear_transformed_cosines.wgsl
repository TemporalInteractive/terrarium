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