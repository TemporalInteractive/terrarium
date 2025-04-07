const TRACE_EPSILON: f32 = 1e-3;

fn safe_origin(origin: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    return origin + normal * TRACE_EPSILON;
}

fn safe_distance(distance: f32) -> f32 {
    return distance - TRACE_EPSILON * 2.0;
}

fn trace_shadow_ray_opaque(origin: vec3<f32>, direction: vec3<f32>, distance: f32, scene: acceleration_structure) -> bool {
    var shadow_rq: ray_query;
    rayQueryInitialize(&shadow_rq, scene, RayDesc(0x4, 0xFFu, 0.0, safe_distance(distance), safe_origin(origin, direction), direction));
    rayQueryProceed(&shadow_rq);
    let intersection = rayQueryGetCommittedIntersection(&shadow_rq);
    return intersection.kind != RAY_QUERY_INTERSECTION_TRIANGLE;
}