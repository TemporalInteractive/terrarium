@include math.wgsl

const FRUSTUM_TILE_SIZE: u32 = 16;

struct Plane {
    normal_and_distance: vec4<f32>,
}

fn Plane::new(p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>) -> Plane {
    let v0: vec3<f32> = p1 - p0;
    let v2: vec3<f32> = p2 - p0;
    let normal: vec3<f32> = normalize(cross(v0, v2));

    let distance: f32 = -dot(normal, p0);
    
    return Plane(vec4<f32>(normal, distance));
}

fn Plane::aabb_outside(_self: Plane, aabb: Aabb) -> bool {
    if ((dot(_self.normal_and_distance, vec4<f32>(aabb.min.x, aabb.min.y, aabb.min.z, 1.0)) < 0.0)
        && (dot(_self.normal_and_distance, vec4<f32>(aabb.max.x, aabb.min.y, aabb.min.z, 1.0)) < 0.0)
        && (dot(_self.normal_and_distance, vec4<f32>(aabb.min.x, aabb.max.y, aabb.min.z, 1.0)) < 0.0)
        && (dot(_self.normal_and_distance, vec4<f32>(aabb.max.x, aabb.max.y, aabb.min.z, 1.0)) < 0.0)
        && (dot(_self.normal_and_distance, vec4<f32>(aabb.min.x, aabb.min.y, aabb.max.z, 1.0)) < 0.0)
        && (dot(_self.normal_and_distance, vec4<f32>(aabb.max.x, aabb.min.y, aabb.max.z, 1.0)) < 0.0)
        && (dot(_self.normal_and_distance, vec4<f32>(aabb.min.x, aabb.max.y, aabb.max.z, 1.0)) < 0.0)
        && (dot(_self.normal_and_distance, vec4<f32>(aabb.max.x, aabb.max.y, aabb.max.z, 1.0)) < 0.0)
    ) {
        return true;
    } else {
        return false;
    }
}

struct Frustum {
    left: Plane,
    right: Plane,
    top: Plane,
    bottom: Plane,
    near: Plane,
    far: Plane,
}

fn Frustum::new(left: Plane, right: Plane, top: Plane, bottom: Plane, near: Plane, far: Plane) -> Frustum {
    return Frustum(left, right, top, bottom, near, far);
}

fn Frustum::intersect_aabb(_self: Frustum, aabb: Aabb) -> bool {
    if (Plane::aabb_outside(_self.left, aabb)) { return false; }
    if (Plane::aabb_outside(_self.right, aabb)) { return false; }
    if (Plane::aabb_outside(_self.top, aabb)) { return false; }
    if (Plane::aabb_outside(_self.bottom, aabb)) { return false; }
    // if (Plane::aabb_outside(_self.near, aabb)) { return false; }
    // if (Plane::aabb_outside(_self.far, aabb)) { return false; }
    return true;
}