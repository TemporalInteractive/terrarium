@include math.wgsl
@include packing.wgsl

struct DebugLineVertex {
    position: vec3<f32>,
    color: PackedRgb9e5,
}

@group(6)
@binding(0)
var<storage, read_write> debug_line_counter: array<atomic<u32>>;

@group(6)
@binding(1)
var<storage, read_write> debug_line_vertices: array<DebugLineVertex>;

fn DebugLines::submit_line(start: vec3<f32>, end: vec3<f32>, color: vec3<f32>) {
    atomicStore(&debug_line_counter[1], 1u);

    let index: u32 = atomicAdd(&debug_line_counter[0], 2u);
    debug_line_vertices[index + 0] = DebugLineVertex(start, PackedRgb9e5::new(color));
    debug_line_vertices[index + 1] = DebugLineVertex(end, PackedRgb9e5::new(color));
}

fn DebugLines::submit_aabb(aabb: Aabb, color: vec3<f32>) {
    let min: vec3<f32> = aabb.min;
    let max: vec3<f32> = aabb.max;

    let p000 = vec3<f32>(min.x, min.y, min.z);
    let p001 = vec3<f32>(min.x, min.y, max.z);
    let p010 = vec3<f32>(min.x, max.y, min.z);
    let p011 = vec3<f32>(min.x, max.y, max.z);
    let p100 = vec3<f32>(max.x, min.y, min.z);
    let p101 = vec3<f32>(max.x, min.y, max.z);
    let p110 = vec3<f32>(max.x, max.y, min.z);
    let p111 = vec3<f32>(max.x, max.y, max.z);

    DebugLines::submit_line(p000, p001, color);
    DebugLines::submit_line(p001, p101, color);
    DebugLines::submit_line(p101, p100, color);
    DebugLines::submit_line(p100, p000, color);
    DebugLines::submit_line(p010, p011, color);
    DebugLines::submit_line(p011, p111, color);
    DebugLines::submit_line(p111, p110, color);
    DebugLines::submit_line(p110, p010, color);
    DebugLines::submit_line(p000, p010, color);
    DebugLines::submit_line(p001, p011, color);
    DebugLines::submit_line(p101, p111, color);
    DebugLines::submit_line(p100, p110, color);
}