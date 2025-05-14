@include shared/frustum.wgsl
@include shared/xr.wgsl

struct Constants {
    resolution: vec2<u32>,
    tile_resolution: vec2<u32>, // tile_resolution = div_ceil(resolution, TILE_SIZE)
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<uniform> xr_camera: XrCamera;

@group(0)
@binding(2)
var<storage, read_write> frustums: array<Frustum>;

@compute
@workgroup_size(FRUSTUM_TILE_SIZE, FRUSTUM_TILE_SIZE)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>, @builtin(num_workgroups) num_groups: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.tile_resolution)) { return; }
    let i: u32 = id.y * constants.tile_resolution.x + id.x;

    let top_left_ss = vec2<f32>(id * FRUSTUM_TILE_SIZE);
    let top_right_ss = vec2<f32>(vec2<u32>(id.x + 1, id.y) * FRUSTUM_TILE_SIZE);
    let bottom_left_ss = vec2<f32>(vec2<u32>(id.x, id.y + 1) * FRUSTUM_TILE_SIZE);
    let bottom_right_ss = vec2<f32>(vec2<u32>(id.x + 1, id.y + 1) * FRUSTUM_TILE_SIZE);

    let top_left_vs: vec3<f32> = XrCamera::screen_to_world_space(xr_camera, top_left_ss, constants.resolution, 0);
    let top_right_vs: vec3<f32> = XrCamera::screen_to_world_space(xr_camera, top_right_ss, constants.resolution, 0);
    let bottom_left_vs: vec3<f32> = XrCamera::screen_to_world_space(xr_camera, bottom_left_ss, constants.resolution, 0);
    let bottom_right_vs: vec3<f32> = XrCamera::screen_to_world_space(xr_camera, bottom_right_ss, constants.resolution, 0);

    let eye = vec3<f32>(0.0);
    let frustum = Frustum::new(
        Plane::new(eye, bottom_left_vs, top_left_vs),
        Plane::new(eye, top_right_vs, bottom_right_vs),
        Plane::new(eye, top_left_vs, top_right_vs),
        Plane::new(eye, bottom_right_vs, bottom_left_vs)
    );

    frustums[i] = frustum;
}