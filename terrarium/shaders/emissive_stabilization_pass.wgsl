@include shared/math.wgsl

struct Constants {
    resolution: vec2<u32>,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var color: texture_storage_2d_array<rgba16float, read_write>;

const SQUARE_GROUP_SIDE: u32 = 8;
const NUM_THREADS_IN_GROUP: u32 = SQUARE_GROUP_SIDE * SQUARE_GROUP_SIDE;
const BORDER_SIZE: i32 = 1;
const SQUARE_GROUPSHARED_SIDE: u32 = SQUARE_GROUP_SIDE + 2 * u32(BORDER_SIZE);

var<workgroup> gs_inputs: array<array<f32, SQUARE_GROUPSHARED_SIDE>, SQUARE_GROUPSHARED_SIDE>;

fn load_inputs(local_thread_index: u32, group_id: vec2<u32>, view_index: u32) {
    let group_top_left_corner: vec2<i32> = vec2<i32>(group_id * SQUARE_GROUP_SIDE) - vec2<i32>(BORDER_SIZE);

    if (view_index > 0) {
        workgroupBarrier();
    }

    for (var i: u32 = local_thread_index; i < SQUARE_GROUPSHARED_SIDE * SQUARE_GROUPSHARED_SIDE; i += NUM_THREADS_IN_GROUP) {
        let group_thread_id = vec2<u32>(i % SQUARE_GROUPSHARED_SIDE, i / SQUARE_GROUPSHARED_SIDE);
        let dispatch_thread_id: vec2<i32> = group_top_left_corner + vec2<i32>(group_thread_id);

        let input: f32 = max(textureLoad(color, dispatch_thread_id, view_index).a, 0.0);
        gs_inputs[group_thread_id.x][group_thread_id.y] = input;
    }

    workgroupBarrier();
}

fn loaded_input(id: vec2<u32>) -> f32 {
    return gs_inputs[id.x][id.y];
}

fn fetchCenterEmission(group_thread_id: vec2<u32>) -> f32 {
    var result: f32 = 0.0;

    for (var y: i32 = -BORDER_SIZE; y <= BORDER_SIZE; y += 1) {
        for (var x: i32 = -BORDER_SIZE; x <= BORDER_SIZE; x += 1) {
            let neigh_pixel = vec2<u32>(vec2<i32>(group_thread_id) + vec2<i32>(x, y) + BORDER_SIZE);
            let neigh: f32 = loaded_input(neigh_pixel);

            result = max(result, neigh);
        }
    }

    return result;
}

@compute
@workgroup_size(SQUARE_GROUP_SIDE, SQUARE_GROUP_SIDE)
fn main(@builtin(global_invocation_id) global_thread_id: vec3<u32>,
    @builtin(local_invocation_index) local_thread_index: u32,
    @builtin(workgroup_id) group_id: vec3<u32>) {
    let id: vec2<u32> = global_thread_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let i: u32 = id.y * constants.resolution.x + id.x;

    let group_thread_id = vec2<u32>(local_thread_index % SQUARE_GROUP_SIDE, local_thread_index / SQUARE_GROUP_SIDE);

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        load_inputs(local_thread_index, group_id.xy, view_index);

        let rgb: vec3<f32> = textureLoad(color, id, view_index).rgb;
        let emission: f32 = fetchCenterEmission(group_thread_id);
        
        textureStore(color, id, view_index, vec4<f32>(rgb, emission));
    }
}