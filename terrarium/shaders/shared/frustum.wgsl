@include math.wgsl

const FRUSTUM_TILE_SIZE: u32 = 16;

struct Plane {
    normal: vec3<f32>,
    distance: f32,
}

fn Plane::new(p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>) -> Plane {
    let v0: vec3<f32> = p1 - p0;
    let v2: vec3<f32> = p2 - p0;
    let normal: vec3<f32> = normalize(cross(v0, v2));

    let distance: f32 = dot(normal, p0);
    
    return Plane(normal, distance);
}

fn Plane::aabb_outside(_self: Plane, aabb: Aabb) -> bool {
    var vertex: vec3<f32> = aabb.min;

    if (_self.normal.x >= 0.0) {
        vertex.x = aabb.max.x;
    }
    if (_self.normal.y >= 0.0) {
        vertex.y = aabb.max.y;
    }
    if (_self.normal.z >= 0.0) {
        vertex.z = aabb.max.z;
    }

    return dot(_self.normal, vertex) > _self.distance;
}

struct Frustum {
    left: Plane,
    right: Plane,
    top: Plane,
    bottom: Plane,
}

fn Frustum::new(left: Plane, right: Plane, top: Plane, bottom: Plane) -> Frustum {
    return Frustum(left, right, top, bottom);
}

fn Frustum::intersect_aabb(_self: Frustum, aabb: Aabb) -> bool {
    if (Plane::aabb_outside(_self.left, aabb)) { return false; }
    if (Plane::aabb_outside(_self.right, aabb)) { return false; }
    if (Plane::aabb_outside(_self.top, aabb)) { return false; }
    if (Plane::aabb_outside(_self.bottom, aabb)) { return false; }
    return true;
}