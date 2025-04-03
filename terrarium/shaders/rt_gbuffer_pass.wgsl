@include terrarium/shaders/shared/xr.wgsl
@include terrarium/shaders/shared/gbuffer.wgsl

@include terrarium/shaders/shared/vertex_pool_bindings.wgsl
@include terrarium/shaders/shared/material_pool_bindings.wgsl

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
var<uniform> xr_camera: XrCamera;

@group(0)
@binding(2)
var scene: acceleration_structure;

@group(0)
@binding(3)
var<storage, read_write> gbuffer_left: array<PackedGBufferTexel>;
@group(0)
@binding(4)
var<storage, read_write> gbuffer_right: array<PackedGBufferTexel>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    var i: u32 = id.y * constants.resolution.x + id.x;

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        let pixel_center = vec2<f32>(id) + xr_camera.jitter;
        var uv: vec2<f32> = (pixel_center / vec2<f32>(constants.resolution)) * 2.0 - 1.0;
        uv.y = -uv.y;
        let origin: vec3<f32> = (xr_camera.view_to_world_space[view_index] * vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz;
        let targt: vec4<f32> = xr_camera.clip_to_view_space[view_index] * vec4<f32>(uv, 1.0, 1.0);
        let direction: vec3<f32> = (xr_camera.view_to_world_space[view_index] * vec4<f32>(normalize(targt.xyz), 0.0)).xyz;

        var depth_ws: f32 = 0.0;
        var normal_ws = vec3<f32>(0.0);
        var tangent_ws = vec3<f32>(0.0);
        var material_descriptor_idx: u32 = 0;
        var tex_coord = vec2<f32>(0.0);
        var velocity = vec2<f32>(0.0);

        var rq: ray_query;
        rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, 1000.0, origin, direction));
        rayQueryProceed(&rq);
        let intersection = rayQueryGetCommittedIntersection(&rq);
        if (intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
            let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[intersection.instance_custom_index];

            let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

            let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
            let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
            let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

            let v0: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i0]);
            let v1: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i1]);
            let v2: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i2]);

            tex_coord = v0.tex_coord * barycentrics.x + v1.tex_coord * barycentrics.y + v2.tex_coord * barycentrics.z;

            material_descriptor_idx = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + intersection.primitive_index];
            let material_descriptor: MaterialDescriptor = material_descriptors[material_descriptor_idx];

            // Load tangent, bitangent and normal in local space
            let tbn: mat3x3<f32> = VertexPoolBindings::load_tbn(v0, v1, v2, barycentrics);

            // Calculate local to world matrix, inversed and transposed
            let local_to_world_inv = mat4x4<f32>(
                vec4<f32>(intersection.world_to_object[0], 0.0),
                vec4<f32>(intersection.world_to_object[1], 0.0),
                vec4<f32>(intersection.world_to_object[2], 0.0),
                vec4<f32>(0.0, 0.0, 0.0, 1.0)
            );
            let local_to_world_inv_trans: mat4x4<f32> = transpose(local_to_world_inv);

            // World space tangent, bitangent and normal. Note that these are not front facing yet
            let hit_tangent_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[0], 1.0)).xyz);
            let hit_bitangent_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[1], 1.0)).xyz);
            var hit_normal_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[2], 1.0)).xyz);
            let hit_point_ws = origin + direction * intersection.t;

            let hit_tangent_to_world = mat3x3<f32>(
                hit_tangent_ws,
                hit_bitangent_ws,
                hit_normal_ws
            );

            // Apply normal mapping when available, unlike the name suggest, still not front facing
            var front_facing_normal_ws: vec3<f32> = hit_normal_ws;
            var front_facing_shading_normal_ws: vec3<f32> = MaterialDescriptor::apply_normal_mapping(material_descriptor, tex_coord, hit_normal_ws, hit_tangent_to_world);

            let w_out_worldspace: vec3<f32> = -direction;

            // Make sure the hit normal and normal mapped normal are front facing
            let back_face: bool = dot(w_out_worldspace, hit_normal_ws) < 0.0;
            if (back_face) {
                front_facing_normal_ws *= -1.0;
                front_facing_shading_normal_ws *= -1.0;
            }

            depth_ws = intersection.t;
            normal_ws = front_facing_shading_normal_ws;
            tangent_ws = hit_tangent_ws;

            let current_position_cs: vec4<f32> = xr_camera.view_to_clip_space[view_index] * xr_camera.world_to_view_space[view_index] * vec4<f32>(hit_point_ws, 1.0);
            let prev_position_cs: vec4<f32> = xr_camera.prev_view_to_clip_space[view_index] * xr_camera.prev_world_to_view_space[view_index] * vec4<f32>(hit_point_ws, 1.0);
            var prev_position_ss: vec4<f32> = (prev_position_cs / prev_position_cs.w + 1.0) / 2.0;
            prev_position_ss = vec4<f32>(prev_position_ss.x, 1.0 - prev_position_ss.y, prev_position_ss.zw);
            var position_ss: vec4<f32> = (current_position_cs / current_position_cs.w + 1.0) / 2.0;
            position_ss = vec4<f32>(position_ss.x, 1.0 - position_ss.y, position_ss.zw);
            velocity = (position_ss - prev_position_ss).xy;
        }

        if (view_index == 0) {
            gbuffer_left[i] = PackedGBufferTexel::new(depth_ws, normal_ws, tangent_ws, material_descriptor_idx, tex_coord, velocity);
        } else {
            gbuffer_right[i] = PackedGBufferTexel::new(depth_ws, normal_ws, tangent_ws, material_descriptor_idx, tex_coord, velocity);
        }
    }
}