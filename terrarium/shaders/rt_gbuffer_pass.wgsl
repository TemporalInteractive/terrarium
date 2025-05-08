@include shared/xr.wgsl
@include shared/gbuffer.wgsl
@include shared/trace.wgsl

@include shared/vertex_pool_bindings.wgsl
@include shared/material_pool_bindings.wgsl

struct Constants {
    resolution: vec2<u32>,
    mipmapping: u32,
    normal_mapping: u32,
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
        let ray: XrCameraRay = XrCamera::raygen(xr_camera, id, constants.resolution, view_index);
        let origin: vec3<f32> = ray.origin;
        let direction: vec3<f32> = ray.direction;

        var position_ws = vec3<f32>(0.0);
        var depth_ws: f32 = 0.0;
        var normal_ws = vec3<f32>(0.0);
        var tangent_ws = vec3<f32>(0.0);
        var material_descriptor_idx: u32 = 0;
        var tex_coord = vec2<f32>(0.0);
        var velocity = vec2<f32>(0.0);
        var ddx = vec2<f32>(0.0);
        var ddy = vec2<f32>(0.0);
        var normal_roughness: f32 = 0.0;
        var geometric_normal_ws = vec3<f32>(0.0);

        var rq: ray_query;
        rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, 10000.0, origin, direction));
        rayQueryProceed(&rq);
        let intersection: RayIntersection = rayQueryGetCommittedIntersection(&rq);
        if (intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
            let vertex_slice_index: u32 = vertex_pool_vertex_slice_indices[intersection.instance_custom_index];
            let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[vertex_slice_index];

            let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

            let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
            let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
            let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

            let v0: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i0]);
            let v1: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i1]);
            let v2: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i2]);

            tex_coord = v0.tex_coord * barycentrics.x + v1.tex_coord * barycentrics.y + v2.tex_coord * barycentrics.z;
            let hit_point: vec3<f32> = v0.position * barycentrics.x + v1.position * barycentrics.y + v2.position * barycentrics.z;

            material_descriptor_idx = VertexPoolBindings::material_idx(intersection.instance_custom_index, vertex_pool_slice.first_index / 3 + intersection.primitive_index);
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
            let hit_normal_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[2], 1.0)).xyz);

            let geometric_normal: vec3<f32> = normalize(cross(v1.position - v0.position, v2.position - v0.position));
            geometric_normal_ws = normalize((local_to_world_inv_trans * vec4<f32>(geometric_normal, 1.0)).xyz);
            let hit_point_ws: vec3<f32> = (intersection.object_to_world * vec4<f32>(hit_point, 1.0)).xyz;
            let prev_hit_point_ws: vec3<f32> = VertexPoolBindings::reproject_point(intersection.instance_custom_index, hit_point_ws);

            let hit_tangent_to_world = mat3x3<f32>(
                hit_tangent_ws,
                hit_bitangent_ws,
                hit_normal_ws
            );

            if (constants.mipmapping > 0) {
                let p0_ws: vec3<f32> = (intersection.object_to_world * vec4<f32>(v0.position, 1.0)).xyz;
                let p1_ws: vec3<f32> = (intersection.object_to_world * vec4<f32>(v1.position, 1.0)).xyz;
                let p2_ws: vec3<f32> = (intersection.object_to_world * vec4<f32>(v2.position, 1.0)).xyz;

                let direction_dx: vec3<f32> = XrCamera::raygen(xr_camera, id + vec2<u32>(1, 0), constants.resolution, view_index).direction;
                let distance_dx: f32 = trace_ray_plane(origin, direction_dx, p0_ws, p1_ws, p2_ws);
                if (distance_dx >= 0.0) {
                    let hit_point_ws_dx: vec3<f32> = origin + direction_dx * distance_dx;
                    let barycentrics_dx: vec3<f32> = VertexPoolBindings::barycentrics_from_point(hit_point_ws_dx, p0_ws, p1_ws, p2_ws);
                    let tex_coord_dx: vec2<f32> = v0.tex_coord * barycentrics_dx.x + v1.tex_coord * barycentrics_dx.y + v2.tex_coord * barycentrics_dx.z;
                    ddx = tex_coord_dx - tex_coord;
                }
                
                let direction_dy: vec3<f32> = XrCamera::raygen(xr_camera, id + vec2<u32>(0, 1), constants.resolution, view_index).direction;
                let distance_dy: f32 = trace_ray_plane(origin, direction_dy, p0_ws, p1_ws, p2_ws);
                if (distance_dy >= 0.0) {
                    let hit_point_ws_dy: vec3<f32> = origin + direction_dy * distance_dy;
                    let barycentrics_dy: vec3<f32> = VertexPoolBindings::barycentrics_from_point(hit_point_ws_dy, p0_ws, p1_ws, p2_ws);
                    let tex_coord_dy: vec2<f32> = v0.tex_coord * barycentrics_dy.x + v1.tex_coord * barycentrics_dy.y + v2.tex_coord * barycentrics_dy.z;
                    ddy = tex_coord_dy - tex_coord;
                }
            }

            // Apply normal mapping when available, unlike the name suggest, not front facing yet
            var mapped_normal_and_roughness: vec4<f32>;
            if (constants.normal_mapping > 0) {
                mapped_normal_and_roughness = MaterialDescriptor::apply_normal_mapping(material_descriptor, tex_coord, ddx, ddy, hit_normal_ws, hit_tangent_to_world);
            } else {
                mapped_normal_and_roughness = vec4<f32>(hit_normal_ws, 1.0);
            }
            var front_facing_shading_normal_ws: vec3<f32> = mapped_normal_and_roughness.xyz;

            let w_out_worldspace: vec3<f32> = -direction;

            // Make sure the hit normal and normal mapped normal are front facing
            let back_face: bool = dot(w_out_worldspace, geometric_normal_ws) < 0.0;
            if (back_face) {
                geometric_normal_ws *= -1.0;
                front_facing_shading_normal_ws *= -1.0;
            }

            position_ws = hit_point_ws;
            depth_ws = intersection.t;
            normal_ws = front_facing_shading_normal_ws;
            normal_roughness = mapped_normal_and_roughness.w;
            tangent_ws = hit_tangent_ws;

            let current_position_cs: vec4<f32> = xr_camera.view_to_clip_space[view_index] * xr_camera.world_to_view_space[view_index] * vec4<f32>(hit_point_ws, 1.0);
            let prev_position_cs: vec4<f32> = xr_camera.prev_view_to_clip_space[view_index] * xr_camera.prev_world_to_view_space[view_index] * vec4<f32>(prev_hit_point_ws, 1.0);
            
            var position_ss: vec4<f32> = (current_position_cs / current_position_cs.w + 1.0) / 2.0;
            position_ss = vec4<f32>(position_ss.x, 1.0 - position_ss.y, position_ss.zw);
            var prev_position_ss: vec4<f32> = (prev_position_cs / prev_position_cs.w + 1.0) / 2.0;
            prev_position_ss = vec4<f32>(prev_position_ss.x, 1.0 - prev_position_ss.y, prev_position_ss.zw);
            velocity = (position_ss - prev_position_ss).xy;
        }

        if (view_index == 0) {
            gbuffer_left[i] = PackedGBufferTexel::new(position_ws, depth_ws, normal_ws, tangent_ws, material_descriptor_idx, tex_coord, velocity, ddx, ddy, normal_roughness, geometric_normal_ws);
        } else {
            gbuffer_right[i] = PackedGBufferTexel::new(position_ws, depth_ws, normal_ws, tangent_ws, material_descriptor_idx, tex_coord, velocity, ddx, ddy, normal_roughness, geometric_normal_ws);
        }
    }
}