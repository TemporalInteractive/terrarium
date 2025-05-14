@include shared/frustum.wgsl
@include shared/math.wgsl

@include shared/gbuffer_bindings.wgsl
@include shared/linear_transformed_cosines_bindings.wgsl
@include shared/debug_line_bindings.wgsl

struct Constants {
    resolution: vec2<u32>,
    tile_resolution: vec2<u32>,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> frustums: array<Frustum>;

@group(0)
@binding(2)
var<storage, read_write> light_index_counter: atomic<u32>;

@group(0)
@binding(3)
var<storage, read_write> light_index_list: array<u32>;

@group(0)
@binding(4)
var light_grid: texture_storage_2d_array<rg32uint, read_write>;

var<workgroup> gs_depth_min: atomic<u32>;
var<workgroup> gs_depth_max: atomic<u32>;

var<workgroup> gs_frustum: Frustum;

var<workgroup> gs_light_count: atomic<u32>;
var<workgroup> gs_light_index_start_offset: atomic<u32>;
var<workgroup> gs_light_list: array<u32, MAX_LTC_INSTANCES_PER_TILE>;

fn append_light(light_index: u32) {
    let index: u32 = atomicAdd(&gs_light_count, 1u);
    if (index < MAX_LTC_INSTANCES_PER_TILE) {
        gs_light_list[index] = light_index;
    }
}

@compute
@workgroup_size(FRUSTUM_TILE_SIZE, FRUSTUM_TILE_SIZE)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>, @builtin(local_invocation_index) local_index: u32,
    @builtin(workgroup_id) group_id: vec3<u32>, @builtin(num_workgroups) num_groups: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }

    let position_and_depth: GbufferPositionAndDepth = Gbuffer::load_position_and_depth(id, 0);
    let depth_u32: u32 = bitcast<u32>(position_and_depth.depth);

    if (local_index == 0) {
        atomicStore(&gs_depth_min, U32_MAX);
        atomicStore(&gs_depth_max, 0u);
        atomicStore(&gs_light_count, 0u);
        gs_frustum = frustums[group_id.y * constants.tile_resolution.x + group_id.x];
    }
    workgroupBarrier();

    atomicMin(&gs_depth_min, depth_u32);
    atomicMax(&gs_depth_max, depth_u32);
    workgroupBarrier();

    let depth_min: f32 = bitcast<f32>(atomicLoad(&gs_depth_min));
    let depth_max: f32 = bitcast<f32>(atomicLoad(&gs_depth_max));

    let eye = XrCamera::origin(xr_camera, 0);
    let top_left_ss = vec2<f32>(id);
    let top_right_ss = vec2<f32>(vec2<u32>(id.x + FRUSTUM_TILE_SIZE, id.y));
    let bottom_left_ss = vec2<f32>(vec2<u32>(id.x, id.y + FRUSTUM_TILE_SIZE));
    let top_left_dir: vec3<f32> = XrCamera::direction(xr_camera, top_left_ss, constants.resolution, 0);
    let top_right_dir: vec3<f32> = XrCamera::direction(xr_camera, top_right_ss, constants.resolution, 0);
    let bottom_left_dir: vec3<f32> = XrCamera::direction(xr_camera, bottom_left_ss, constants.resolution, 0);

    let top_left_near_ws = eye + top_left_dir * depth_min;
    let top_right_near_ws = eye + top_right_dir * depth_min;
    let bottom_left_near_ws = eye + bottom_left_dir * depth_min;
    let near_plane = Plane::new(top_left_near_ws, top_right_near_ws, bottom_left_near_ws);

    let top_left_far_ws = eye + top_left_dir * depth_max;
    let top_right_far_ws = eye + top_right_dir * depth_max;
    let bottom_left_far_ws = eye + bottom_left_dir * depth_max;
    let far_plane = Plane::new(top_left_far_ws, top_right_far_ws, bottom_left_far_ws);

    for (var i: u32 = local_index; i < ltc_constants.instance_count; i += FRUSTUM_TILE_SIZE * FRUSTUM_TILE_SIZE) {
        let light: LtcInstance = ltc_instances[i];
        let aabb: Aabb = LtcInstance::illuminated_aabb(light);

        let culled: bool = !Frustum::intersect_aabb(gs_frustum, aabb);

        if (group_id.x == 40 && group_id.y == 40) {
            if culled {
                DebugLines::submit_aabb(aabb, vec3<f32>(1.0, 0.0, 1.0));
            } else {
                DebugLines::submit_aabb(aabb, vec3<f32>(0.0, 1.0, 0.0));
            }
        }

        

        if (!culled) {
            append_light(i);
        }
    }
    workgroupBarrier();

    let light_count: u32 = min(atomicLoad(&gs_light_count), MAX_LTC_INSTANCES_PER_TILE);
    if (local_index == 0) {
        let light_index_start_offset: u32 = atomicAdd(&light_index_counter, light_count);
        atomicStore(&gs_light_index_start_offset, light_index_start_offset);

        textureStore(light_grid, group_id.xy, 0, vec4<u32>(light_index_start_offset, light_count, 0, 0));
    }
    workgroupBarrier();

    let light_index_start_offset: u32 = atomicLoad(&gs_light_index_start_offset);
    for (var i: u32 = local_index; i < light_count; i += FRUSTUM_TILE_SIZE * FRUSTUM_TILE_SIZE) {
        light_index_list[light_index_start_offset + i] = gs_light_list[i];
    }
}