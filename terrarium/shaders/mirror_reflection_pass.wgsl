@include shared/xr.wgsl
@include shared/trace.wgsl

@include shared/vertex_pool_bindings.wgsl
@include shared/material_pool_bindings.wgsl
@include shared/sky_bindings.wgsl
@include shared/gbuffer_bindings.wgsl

struct Constants {
    resolution: vec2<u32>,
    reflection_resolution: vec2<u32>,
    ambient_factor: f32,
    view_index: u32,
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
var static_scene: acceleration_structure;

@group(0)
@binding(3)
var dynamic_scene: acceleration_structure;

@group(0)
@binding(4)
var reflection_out: texture_storage_2d_array<rgba16float, read_write>;

@group(0)
@binding(5)
var<storage, read> reflection_counter: u32;

@group(0)
@binding(6)
var<storage, read> reflection_pid: array<u32>;

fn trace_ray(origin: vec3<f32>, direction: vec3<f32>) -> RayIntersection {
    var rq: ray_query;

    rayQueryInitialize(&rq, static_scene, RayDesc(0u, 0xFFu, 0.0, 10000.0, origin, direction));
    rayQueryProceed(&rq);
    let static_intersection: RayIntersection = rayQueryGetCommittedIntersection(&rq);
    var static_t: f32 = 10000.0;
    if (static_intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
        static_t = static_intersection.t;
    }

    rayQueryInitialize(&rq, dynamic_scene, RayDesc(0u, 0xFFu, 0.0, 10000.0, origin, direction));
    rayQueryProceed(&rq);
    let dynamic_intersection: RayIntersection = rayQueryGetCommittedIntersection(&rq);
    var dynamic_t: f32 = 10000.0;
    if (dynamic_intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
        dynamic_t = dynamic_intersection.t;
    }
    
    if (static_t < dynamic_t) {
        return static_intersection;
    } else {
        return dynamic_intersection;
    }
}

@compute
@workgroup_size(128)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var i: u32 = global_id.x;
    if (i >= reflection_counter) { return; }

    let view_index: u32 = constants.view_index;
    let pid: u32 = reflection_pid[i];
    let id = vec2<u32>(pid % constants.resolution.x, pid / constants.resolution.x);

    let position_and_depth: GbufferPositionAndDepth = Gbuffer::load_position_and_depth(id, view_index);

    var reflection = vec3<f32>(0.0);
    if (!GbufferPositionAndDepth::is_sky(position_and_depth)) {
        let shading_and_geometric_normal: GbufferShadingAndGeometricNormal = Gbuffer::load_shading_and_geometric_normal(id, view_index);
        let ray: XrCameraRay = XrCamera::raygen(xr_camera, id, constants.resolution, view_index);

        let origin: vec3<f32> = position_and_depth.position + shading_and_geometric_normal.geometric_normal * 0.001;
        let direction: vec3<f32> = reflect(ray.direction, shading_and_geometric_normal.geometric_normal);

        let intersection: RayIntersection = trace_ray(origin, direction);
        if (intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
            let tex_coord_and_derivatives: GbufferTexCoordAndDerivatives = Gbuffer::load_tex_coord_and_derivatives(id, view_index);

            let vertex_slice_index: u32 = vertex_pool_vertex_slice_indices[intersection.instance_custom_data];
            let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[vertex_slice_index];

            let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

            let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
            let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
            let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

            let v0: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i0]);
            let v1: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i1]);
            let v2: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i2]);

            let tex_coord: vec2<f32> = v0.tex_coord * barycentrics.x + v1.tex_coord * barycentrics.y + v2.tex_coord * barycentrics.z;

            let material_descriptor_idx: u32 = VertexPoolBindings::material_idx(intersection.instance_custom_data, vertex_pool_slice.first_index / 3 + intersection.primitive_index);
            let material_descriptor: MaterialDescriptor = material_descriptors[material_descriptor_idx];
            let material: Material = Material::from_material_descriptor(material_descriptor, tex_coord, tex_coord_and_derivatives.ddx, tex_coord_and_derivatives.ddy);

            reflection = material.emission + material.color * constants.ambient_factor;
        } else {
            reflection = Sky::inscattering(direction, false);
        }
    }

    textureStore(reflection_out, id, view_index, vec4<f32>(reflection, 1.0));
}