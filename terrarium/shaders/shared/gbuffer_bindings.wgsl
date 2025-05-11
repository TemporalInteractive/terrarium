@include packing.wgsl

@group(4)
@binding(0)
// RGB: position (vec3<f32>)
// A: depth (f32)
var gbuffer_position_and_depth: texture_storage_2d_array<rgba32float, read_write>;

@group(4)
@binding(1)
// R: shading_normal (PackedNormalizedXyz10)
// G: geometric_normal (PackedNormalizedXyz10)
var gbuffer_shading_and_geometric_normal: texture_storage_2d_array<rg32uint, read_write>;

@group(4)
@binding(2)
// RG: tex_coord (vec2<f32>)
// B: ddx (vec2<f16>)
// A: ddy (vec2<f16>)
var gbuffer_tex_coord_and_derivatives: texture_storage_2d_array<rgba32float, read_write>;

@group(4)
@binding(3)
// RG: velocity (vec2<f32>)
var gbuffer_velocity: texture_storage_2d_array<rg32float, read_write>;

@group(4)
@binding(4)
// R: material_descriptor_idx (u32)
// G: normal_roughness (f32)
var gbuffer_material_descriptor_idx_and_normal_roughness: texture_storage_2d_array<rg32float, read_write>;

struct GbufferPositionAndDepth {
    position: vec3<f32>,
    depth: f32,
}

fn GbufferPositionAndDepth::is_sky(_self: GbufferPositionAndDepth) -> bool {
    return _self.depth == 0.0;
}

struct GbufferShadingAndGeometricNormal {
    shading_normal: vec3<f32>,
    geometric_normal: vec3<f32>,
}

struct GbufferTexCoordAndDerivatives {
    tex_coord: vec2<f32>,
    ddx: vec2<f32>,
    ddy: vec2<f32>,
}

struct GbufferMaterialDescriptorIdxAndNormalRoughness {
    material_descriptor_idx: u32,
    normal_roughness: f32,
}

fn Gbuffer::load_position_and_depth(id: vec2<u32>, view_index: u32) -> GbufferPositionAndDepth {
    let data: vec4<f32> = textureLoad(gbuffer_position_and_depth, id, view_index);

    return GbufferPositionAndDepth(data.rgb, data.a);
}

fn Gbuffer::store_position_and_depth(position: vec3<f32>, depth: f32, id: vec2<u32>, view_index: u32) {
    let data = vec4<f32>(position, depth);

    textureStore(gbuffer_position_and_depth, id, view_index, data);
}

fn Gbuffer::load_shading_and_geometric_normal(id: vec2<u32>, view_index: u32) -> GbufferShadingAndGeometricNormal {
    let data: vec2<u32> = textureLoad(gbuffer_shading_and_geometric_normal, id, view_index).rg;

    return GbufferShadingAndGeometricNormal(
        PackedNormalizedXyz10::unpack(PackedNormalizedXyz10(data.x), 0),
        PackedNormalizedXyz10::unpack(PackedNormalizedXyz10(data.y), 0)
    );
}

fn Gbuffer::store_shading_and_geometric_normal(shading_normal: vec3<f32>, geometric_normal: vec3<f32>, id: vec2<u32>, view_index: u32) {
    let data = vec2<u32>(
        PackedNormalizedXyz10::new(shading_normal, 0).data,
        PackedNormalizedXyz10::new(geometric_normal, 0).data
    );

    textureStore(gbuffer_shading_and_geometric_normal, id, view_index, vec4<u32>(data, 0, 0));
}

fn Gbuffer::load_tex_coord_and_derivatives(id: vec2<u32>, view_index: u32) -> GbufferTexCoordAndDerivatives {
    let data: vec4<f32> = textureLoad(gbuffer_tex_coord_and_derivatives, id, view_index);

    return GbufferTexCoordAndDerivatives(
        data.rg,
        unpack2x16float(bitcast<u32>(data.b)),
        unpack2x16float(bitcast<u32>(data.a))
    );
}

fn Gbuffer::store_tex_coord_and_derivatives(tex_coord: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>, id: vec2<u32>, view_index: u32) {
    let data = vec4<f32>(
        tex_coord,
        bitcast<f32>(pack2x16float(ddx)),
        bitcast<f32>(pack2x16float(ddy))
    );

    textureStore(gbuffer_tex_coord_and_derivatives, id, view_index, data);
}

fn Gbuffer::load_velocity(id: vec2<u32>, view_index: u32) -> vec2<f32> {
    return textureLoad(gbuffer_velocity, id, view_index).rg;
}

fn Gbuffer::store_velocity(velocity: vec2<f32>, id: vec2<u32>, view_index: u32) {
    textureStore(gbuffer_velocity, id, view_index, vec4<f32>(velocity, 0.0, 0.0));
}

fn Gbuffer::load_material_descriptor_idx_and_normal_roughness(id: vec2<u32>, view_index: u32) -> GbufferMaterialDescriptorIdxAndNormalRoughness {
    let data: vec2<f32> = textureLoad(gbuffer_material_descriptor_idx_and_normal_roughness, id, view_index).rg;

    return GbufferMaterialDescriptorIdxAndNormalRoughness(
        bitcast<u32>(data.r),
        data.g
    );
}

fn Gbuffer::store_material_descriptor_idx_and_normal_roughness(material_descriptor_idx: u32, normal_roughness: f32, id: vec2<u32>, view_index: u32) {
    let data = vec2<f32>(
        bitcast<f32>(material_descriptor_idx),
        normal_roughness
    );

    textureStore(gbuffer_material_descriptor_idx_and_normal_roughness, id, view_index, vec4<f32>(data, 0.0, 0.0));
}