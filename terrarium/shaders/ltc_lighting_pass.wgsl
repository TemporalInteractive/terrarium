@include shared/brdf.wgsl
@include shared/frustum.wgsl
@include shared/xr.wgsl

@include shared/vertex_pool_bindings.wgsl
@include shared/material_pool_bindings.wgsl
@include shared/sky_bindings.wgsl
@include shared/gbuffer_bindings.wgsl
@include shared/linear_transformed_cosines_bindings.wgsl

struct Constants {
    resolution: vec2<u32>,
    lighting_resolution: vec2<u32>,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<uniform> xr_camera: XrCamera;

@group(0)
@binding(4)
var lighting_out: texture_storage_2d_array<rgba16float, read_write>;

@group(0)
@binding(5)
var<storage, read> light_index_list: array<u32>;

@group(0)
@binding(6)
var light_grid: texture_storage_2d<rg32uint, read>;

@compute
@workgroup_size(FRUSTUM_TILE_SIZE, FRUSTUM_TILE_SIZE)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>, @builtin(num_workgroups) num_groups: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (any(id >= constants.lighting_resolution)) { return; }

    let full_res_id: vec2<u32> = vec2<u32>(vec2<f32>(id) * (vec2<f32>(constants.resolution) / vec2<f32>(constants.lighting_resolution)));

    // TODO: move to groupshared?
    let light_offset_and_count: vec2<u32> = textureLoad(light_grid, group_id.xy).rg;
    let light_index_start_offset: u32 = light_offset_and_count.x;
    let light_count: u32 = light_offset_and_count.y;

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        let ray: XrCameraRay = XrCamera::raygen(xr_camera, full_res_id, constants.resolution, view_index);

        let position_and_depth: GbufferPositionAndDepth = Gbuffer::load_position_and_depth(full_res_id, view_index);
        var emission: f32 = 0.0;

        var lighting = vec3<f32>(0.0);
        if (!GbufferPositionAndDepth::is_sky(position_and_depth)) {
            let material_descriptor_idx_and_normal_roughness: GbufferMaterialDescriptorIdxAndNormalRoughness
                = Gbuffer::load_material_descriptor_idx_and_normal_roughness(full_res_id, view_index);
            let tex_coord_and_derivatives: GbufferTexCoordAndDerivatives = Gbuffer::load_tex_coord_and_derivatives(full_res_id, view_index);
            let shading_and_geometric_normal: GbufferShadingAndGeometricNormal = Gbuffer::load_shading_and_geometric_normal(full_res_id, view_index);

            let material_descriptor: MaterialDescriptor = material_descriptors[material_descriptor_idx_and_normal_roughness.material_descriptor_idx];
            var material: Material = Material::from_material_descriptor(material_descriptor, tex_coord_and_derivatives.tex_coord, tex_coord_and_derivatives.ddx, tex_coord_and_derivatives.ddy);

            let geometric_roughness: f32 = safe_sqrt(1.0 - material_descriptor_idx_and_normal_roughness.normal_roughness);
            material.roughness = safe_sqrt(sqr(material.roughness) + sqr(geometric_roughness));

            for (var local_light_index: u32 = 0; local_light_index < light_count; local_light_index += 1) {
                let light_index: u32 = light_index_list[light_index_start_offset + local_light_index];
                lighting += LtcBindings::shade(material, light_index, shading_and_geometric_normal.shading_normal, -ray.direction, position_and_depth.position);
            }
        }

        textureStore(lighting_out, id, view_index, vec4<f32>(lighting, 1.0));
    }
}