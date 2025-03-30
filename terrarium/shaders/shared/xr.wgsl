struct XrCamera {
    view_to_clip_space: array<mat4x4<f32>, 2>,
    world_to_view_space: array<mat4x4<f32>, 2>,
    clip_to_view_space: array<mat4x4<f32>, 2>,
    view_to_world_space: array<mat4x4<f32>, 2>,
}