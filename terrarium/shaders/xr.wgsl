struct XrCamera {
    stage_to_clip_space: array<mat4x4<f32>, 2>,
    world_to_stage_space: mat4x4<f32>,
}