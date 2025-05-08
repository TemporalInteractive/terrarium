@include vertex_pool.wgsl

const MAX_DYNAMIC_INSTANCES: u32 = 1024 * 2;
const MAX_MATERIALS_PER_INSTANCE: u32 = 4;

@group(1)
@binding(1)
var<storage, read> vertices: array<PackedVertex>;

@group(1)
@binding(2)
var<storage, read> vertex_indices: array<u32>;

@group(1)
@binding(3)
var<storage, read> triangle_material_indices: array<u32>;

@group(1)
@binding(4)
var<storage, read> vertex_pool_slices: array<VertexPoolSlice>;

@group(1)
@binding(5)
var<storage, read> delta_object_to_world_inv: array<mat4x4<f32>>;

@group(1)
@binding(6)
var<storage, read> vertex_pool_material_indices: array<u32>;

@group(1)
@binding(7)
var<storage, read> vertex_pool_vertex_slice_indices: array<u32>;

fn _calculate_bitangent(normal: vec3<f32>, tangent: vec4<f32>) -> vec3<f32> {
    var bitangent: vec3<f32> = cross(normal, tangent.xyz);
    return bitangent * -tangent.w;
}

fn VertexPoolBindings::load_tbn(v0: Vertex, v1: Vertex, v2: Vertex, barycentrics: vec3<f32>) -> mat3x3<f32> {
    let normal: vec3<f32> = v0.normal * barycentrics.x + v1.normal * barycentrics.y + v2.normal * barycentrics.z;
    let tangent: vec3<f32> = v0.tangent.xyz * barycentrics.x + v1.tangent.xyz * barycentrics.y + v2.tangent.xyz * barycentrics.z;

    let bitangent0: vec3<f32> = _calculate_bitangent(v0.normal, v0.tangent);
    let bitangent1: vec3<f32> = _calculate_bitangent(v1.normal, v1.tangent);
    let bitangent2: vec3<f32> = _calculate_bitangent(v2.normal, v2.tangent);
    let bitangent: vec3<f32> = bitangent0 * barycentrics.x + bitangent1 * barycentrics.y + bitangent2 * barycentrics.z;

    return mat3x3<f32>(tangent, bitangent, normal);
}

fn VertexPoolBindings::barycentrics_from_point(point: vec3<f32>, p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>) -> vec3<f32> {
    let v0: vec3<f32> = p1 - p0;
    let v1: vec3<f32> = p2 - p0;
    let v2: vec3<f32> = point - p0;

    let d00: f32 = dot(v0, v0);
    let d01: f32 = dot(v0, v1);
    let d11: f32 = dot(v1, v1);
    let d20: f32 = dot(v2, v0);
    let d21: f32 = dot(v2, v1);

    let denom: f32 = d00 * d11 - d01 * d01;
    let v: f32 = (d11 * d20 - d01 * d21) / denom;
    let w: f32 = (d00 * d21 - d01 * d20) / denom;
    let u: f32 = 1.0 - v - w;

    return vec3<f32>(u, v, w);
}

fn VertexPoolBindings::material_idx(instance_idx: u32, triangle_idx: u32) -> u32 {
    let triangle_material_idx: u32 = triangle_material_indices[triangle_idx];
    return vertex_pool_material_indices[instance_idx * MAX_MATERIALS_PER_INSTANCE + triangle_material_idx];
}

fn VertexPoolBindings::reproject_point(instance_idx: u32, point: vec3<f32>) -> vec3<f32> {
    if (instance_idx < MAX_DYNAMIC_INSTANCES) {
        let delta: mat4x4<f32> = delta_object_to_world_inv[instance_idx];
        return (delta * vec4<f32>(point, 1.0)).xyz;
    } else {
        return point;
    }
}