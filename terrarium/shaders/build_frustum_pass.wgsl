@include shared/frustum.wgsl
@include shared/xr.wgsl

@include shared/gbuffer_bindings.wgsl

struct Constants {
    resolution: vec2<u32>,
    lighting_resolution: vec2<u32>,
    tile_resolution: vec2<u32>, // tile_resolution = div_ceil(resolution, TILE_SIZE)
    _padding0: u32,
    _padding1: u32,
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

var<workgroup> gs_depth_min: atomic<u32>;
var<workgroup> gs_depth_max: atomic<u32>;

@compute
@workgroup_size(FRUSTUM_TILE_SIZE, FRUSTUM_TILE_SIZE)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>, @builtin(num_workgroups) num_groups: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.lighting_resolution)) { return; }

    let lighting_res_scale: vec2<f32> = vec2<f32>(constants.resolution) / vec2<f32>(constants.lighting_resolution);
    let full_res_id: vec2<u32> = vec2<u32>(vec2<f32>(id) * lighting_res_scale);

    if (local_id.x == 0 && local_id.y == 0) {
        atomicStore(&gs_depth_min, U32_MAX);
        atomicStore(&gs_depth_max, 0u);
    }
    workgroupBarrier();

    let position_and_depth: GbufferPositionAndDepth = Gbuffer::load_position_and_depth(full_res_id, 0);
    if (!GbufferPositionAndDepth::is_sky(position_and_depth)) {
        let depth_u32: u32 = bitcast<u32>(position_and_depth.depth);
        atomicMin(&gs_depth_min, depth_u32);
        atomicMax(&gs_depth_max, depth_u32);
    }
    workgroupBarrier();

    let depth_min: f32 = bitcast<f32>(atomicLoad(&gs_depth_min));
    let depth_max: f32 = bitcast<f32>(atomicLoad(&gs_depth_max));

    if (local_id.x == 0 && local_id.y == 0) {
        let i: u32 = group_id.y * constants.tile_resolution.x + group_id.x;

        let top_left_ss = vec2<f32>(group_id.xy * FRUSTUM_TILE_SIZE) * lighting_res_scale;
        let top_right_ss = vec2<f32>(vec2<u32>(group_id.x + 1, group_id.y) * FRUSTUM_TILE_SIZE) * lighting_res_scale;
        let bottom_left_ss = vec2<f32>(vec2<u32>(group_id.x, group_id.y + 1) * FRUSTUM_TILE_SIZE) * lighting_res_scale;
        let bottom_right_ss = vec2<f32>(vec2<u32>(group_id.x + 1, group_id.y + 1) * FRUSTUM_TILE_SIZE) * lighting_res_scale;

        let eye: vec3<f32> = (XrCamera::origin(xr_camera, 0) + XrCamera::origin(xr_camera, 1)) / 2.0;

        let top_left_dir: vec3<f32> = (XrCamera::direction(xr_camera, top_left_ss, constants.resolution, 0) + XrCamera::direction(xr_camera, top_left_ss, constants.resolution, 1)) / 2.0;
        let top_right_dir: vec3<f32> = (XrCamera::direction(xr_camera, top_right_ss, constants.resolution, 0) + XrCamera::direction(xr_camera, top_right_ss, constants.resolution, 1)) / 2.0;
        let bottom_left_dir: vec3<f32> = (XrCamera::direction(xr_camera, bottom_left_ss, constants.resolution, 0) + XrCamera::direction(xr_camera, bottom_left_ss, constants.resolution, 1)) / 2.0;
        let bottom_right_dir: vec3<f32> = (XrCamera::direction(xr_camera, bottom_right_ss, constants.resolution, 0) + XrCamera::direction(xr_camera, bottom_right_ss, constants.resolution, 1)) / 2.0;

        let top_left_near_ws: vec3<f32> = eye + top_left_dir * depth_min;
        let top_right_near_ws: vec3<f32> = eye + top_right_dir * depth_min;
        let bottom_left_near_ws: vec3<f32> = eye + bottom_left_dir * depth_min;
        let bottom_right_near_ws: vec3<f32> = eye + bottom_right_dir * depth_min;

        let top_left_far_ws: vec3<f32> = eye + top_left_dir * depth_max;
        let top_right_far_ws: vec3<f32> = eye + top_right_dir * depth_max;
        let bottom_left_far_ws: vec3<f32> = eye + bottom_left_dir * depth_max;
        let bottom_right_far_ws: vec3<f32> = eye + bottom_right_dir * depth_max;
        
        let frustum = Frustum::new(
            Plane::new(eye, bottom_left_near_ws, top_left_near_ws),
            Plane::new(eye, top_right_near_ws, bottom_right_near_ws),
            Plane::new(eye, top_left_near_ws, top_right_near_ws),
            Plane::new(eye, bottom_right_near_ws, bottom_left_near_ws),
            Plane::new(bottom_left_near_ws, top_left_near_ws, top_right_near_ws),
            Plane::new(top_right_far_ws, bottom_left_far_ws, bottom_right_far_ws)
        );

        frustums[i] = frustum;
    }
}