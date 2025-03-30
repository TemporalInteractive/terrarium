@include terrarium/shaders/shared/packing.wgsl

struct VertexPoolSlice {
    first_vertex: u32,
    num_vertices: u32,
    first_index: u32,
    num_indices: u32,
    material_idx: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
};

struct EmissiveTriangleInstance { // EmissiveBlasInstance
    trans_transform: mat3x4<f32>, // Transposed for memory alignment
    vertex_pool_slice_idx: u32,
    num_triangles: u32,
    _padding0: u32,
    _padding1: u32,
}

struct BlasInstance {
    emissive_blas_instance_idx: u32,
    vertex_pool_slice_index: u32,
    _padding0: u32,
    _padding1: u32,
}

fn BlasInstance::is_emissive(_self: BlasInstance) -> bool {
    return _self.emissive_blas_instance_idx != U32_MAX;
}

struct Vertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    tex_coord: vec2<f32>,
    tangent: vec4<f32>,
};

struct PackedVertex {
    position: vec3<f32>,
    normal: PackedNormalizedXyz10,
    tex_coord: vec2<f32>,
    tangent: PackedNormalizedXyz10,
    tangent_handiness: f32,
};

fn PackedVertex::unpack(_self: PackedVertex) -> Vertex {
    return Vertex(
        _self.position,
        PackedNormalizedXyz10::unpack(_self.normal, 0),
        _self.tex_coord,
        vec4<f32>(PackedNormalizedXyz10::unpack(_self.tangent, 0), _self.tangent_handiness)
    );
}

fn VertexPool::calculate_bitangent(normal: vec3<f32>, tangent: vec3<f32>, tangent_handiness: f32) -> vec3<f32> {
    var bitangent: vec3<f32> = cross(normal, tangent);
    return bitangent * tangent_handiness;
}