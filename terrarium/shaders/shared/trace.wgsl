const TRACE_EPSILON: f32 = 1e-3;

fn safe_origin(origin: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    return origin + normal * TRACE_EPSILON;
}

fn safe_distance(distance: f32) -> f32 {
    return distance - TRACE_EPSILON * 2.0;
}

fn trace_shadow_ray_opaque(origin: vec3<f32>, direction: vec3<f32>, distance: f32, normal_ws: vec3<f32>, scene: acceleration_structure) -> bool {
    var shadow_rq: ray_query;
    rayQueryInitialize(&shadow_rq, scene, RayDesc(0x4, 0xFFu, 0.0, safe_distance(distance), safe_origin(origin, normal_ws), direction));
    rayQueryProceed(&shadow_rq);
    let intersection = rayQueryGetCommittedIntersection(&shadow_rq);
    return intersection.kind != RAY_QUERY_INTERSECTION_TRIANGLE;
}

fn trace_ray_plane(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
    p0: vec3<f32>,
    p1: vec3<f32>,
    p2: vec3<f32>
) -> f32 {
    let edge1: vec3<f32> = p1 - p0;
    let edge2: vec3<f32> = p2 - p0;
    let normal: vec3<f32> = normalize(cross(edge1, edge2));

    let denom: f32 = dot(ray_dir, normal);
    if abs(denom) < 1e-6 {
        return -1.0;
    }

    let t: f32 = dot(p0 - ray_origin, normal) / denom;
    if t < 0.0 {
        return -1.0;
    }

    return t;
}