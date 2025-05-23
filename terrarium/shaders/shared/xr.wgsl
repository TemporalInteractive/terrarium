struct XrCamera {
    view_to_clip_space: array<mat4x4<f32>, 2>,
    world_to_view_space: array<mat4x4<f32>, 2>,
    clip_to_view_space: array<mat4x4<f32>, 2>,
    view_to_world_space: array<mat4x4<f32>, 2>,
    jitter: vec2<f32>,
    _padding0: u32,
    _padding1: u32,
    prev_view_to_clip_space: array<mat4x4<f32>, 2>,
    prev_world_to_view_space: array<mat4x4<f32>, 2>,
    prev_clip_to_view_space: array<mat4x4<f32>, 2>,
    prev_view_to_world_space: array<mat4x4<f32>, 2>,
    prev_jitter: vec2<f32>,
    _padding2: u32,
    _padding3: u32,
}

struct XrCameraRay {
    origin: vec3<f32>,
    direction: vec3<f32>,
}

fn XrCamera::raygen(xr_camera: XrCamera, id: vec2<u32>, resolution: vec2<u32>, view_index: u32) -> XrCameraRay {
    let pixel_center = vec2<f32>(id) + vec2<f32>(0.5) + xr_camera.jitter;
    let direction: vec3<f32> = XrCamera::direction(xr_camera, pixel_center, resolution, view_index);
    let origin: vec3<f32> = XrCamera::origin(xr_camera, view_index);

    return XrCameraRay(origin, direction);
}

fn XrCamera::origin(xr_camera: XrCamera, view_index: u32) -> vec3<f32> {
    return (xr_camera.view_to_world_space[view_index] * vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz;
}

fn XrCamera::direction(xr_camera: XrCamera, screen_space: vec2<f32>, resolution: vec2<u32>, view_index: u32) -> vec3<f32> {
    var uv: vec2<f32> = (screen_space / vec2<f32>(resolution)) * 2.0 - 1.0;
    uv.y = -uv.y;
    let targt: vec3<f32> = (xr_camera.clip_to_view_space[view_index] * vec4<f32>(uv, 1.0, 1.0)).xyz;
    return (xr_camera.view_to_world_space[view_index] * vec4<f32>(normalize(targt), 0.0)).xyz;
}
