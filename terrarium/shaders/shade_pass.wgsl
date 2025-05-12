@include shared/brdf.wgsl
@include shared/xr.wgsl

@include shared/vertex_pool_bindings.wgsl
@include shared/material_pool_bindings.wgsl
@include shared/sky_bindings.wgsl
@include shared/gbuffer_bindings.wgsl

const SHADING_MODE_FULL: u32 = 0;
const SHADING_MODE_LIGHTING_ONLY: u32 = 1;
const SHADING_MODE_ALBEDO: u32 = 2;
const SHADING_MODE_NORMALS: u32 = 3;
const SHADING_MODE_TEX_COORDS: u32 = 4;
const SHADING_MODE_EMISSION: u32 = 5;
const SHADING_MODE_VELOCITY: u32 = 6;
const SHADING_MODE_FOG: u32 = 7;
const SHADING_MODE_SIMPLE_LIGHTING: u32 = 8;

struct Constants {
    resolution: vec2<u32>,
    shading_mode: u32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<uniform> xr_camera: XrCamera;

@group(0)
@binding(2)
var shadow: texture_2d_array<f32>;
@group(0)
@binding(3)
var shadow_sampler: sampler;

@group(0)
@binding(4)
var color_out: texture_storage_2d_array<rgba16float, read_write>;

fn shade_fog(shade_color: vec3<f32>, position_and_depth: GbufferPositionAndDepth, view_origin: vec3<f32>, view_dir: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    let density: f32 = sky_constants.atmosphere.density * Sky::atmosphere_density(view_origin, position_and_depth.position);
    let fog_strength: f32 = 1.0 - exp(-position_and_depth.depth * density);
    let inscattering: vec3<f32> = Sky::inscattering(view_dir, true);
    return mix(shade_color, inscattering, fog_strength);
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    var i: u32 = id.y * constants.resolution.x + id.x;

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        let ray: XrCameraRay = XrCamera::raygen(xr_camera, id, constants.resolution, view_index);

        let position_and_depth: GbufferPositionAndDepth = Gbuffer::load_position_and_depth(id, view_index);
        var emission: f32 = 0.0;

        var color: vec3<f32>;
        if (!GbufferPositionAndDepth::is_sky(position_and_depth)) {
            let material_descriptor_idx_and_normal_roughness: GbufferMaterialDescriptorIdxAndNormalRoughness
                = Gbuffer::load_material_descriptor_idx_and_normal_roughness(id, view_index);
            let tex_coord_and_derivatives: GbufferTexCoordAndDerivatives = Gbuffer::load_tex_coord_and_derivatives(id, view_index);
            let shading_and_geometric_normal: GbufferShadingAndGeometricNormal = Gbuffer::load_shading_and_geometric_normal(id, view_index);

            let material_descriptor: MaterialDescriptor = material_descriptors[material_descriptor_idx_and_normal_roughness.material_descriptor_idx];
            var material: Material = Material::from_material_descriptor(material_descriptor, tex_coord_and_derivatives.tex_coord, tex_coord_and_derivatives.ddx, tex_coord_and_derivatives.ddy);

            let geometric_roughness: f32 = safe_sqrt(1.0 - material_descriptor_idx_and_normal_roughness.normal_roughness);
            material.roughness = safe_sqrt(sqr(material.roughness) + sqr(geometric_roughness));

            emission = select(0.0, 1.0, dot(material.emission, material.emission) > 0.0);

            if (constants.shading_mode == SHADING_MODE_SIMPLE_LIGHTING) {
                let l: vec3<f32> = sky_constants.world_up;
                let n_dot_l: f32 = max(dot(shading_and_geometric_normal.shading_normal, l), 0.0);

                let reflectance: vec3<f32> = Material::eval_brdf(material, l, -ray.direction, shading_and_geometric_normal.shading_normal);

                let ambient: vec3<f32> = material.color * 0.1;

                color = reflectance * n_dot_l + ambient + material.emission;
            } else {
                let shadow: f32 = 1.0 - textureSampleLevel(shadow, shadow_sampler, (vec2<f32>(id) + vec2<f32>(0.5)) / vec2<f32>(constants.resolution), view_index, 0.0).r;

                let l: vec3<f32> = -sky_constants.sun.direction;
                let n_dot_l: f32 = max(dot(shading_and_geometric_normal.shading_normal, l), 0.0);

                let light_intensity: f32 = shadow * Sky::sun_intensity(l);
                let reflectance: vec3<f32> = Material::eval_brdf(material, l, -ray.direction, shading_and_geometric_normal.shading_normal);

                let ambient: vec3<f32> = material.color * 0.1;

                if (constants.shading_mode == SHADING_MODE_FULL) {
                    color = reflectance * light_intensity * n_dot_l + ambient + material.emission;
                    color = shade_fog(color, position_and_depth, ray.origin, ray.direction, l);
                } else if (constants.shading_mode == SHADING_MODE_LIGHTING_ONLY) {
                    color = vec3<f32>(light_intensity * n_dot_l) + ambient;
                } else if (constants.shading_mode == SHADING_MODE_ALBEDO) {
                    color = material.color;
                } else if (constants.shading_mode == SHADING_MODE_NORMALS) {
                    color = shading_and_geometric_normal.shading_normal * 0.5 + 0.5;
                } else if (constants.shading_mode == SHADING_MODE_TEX_COORDS) {
                    color = vec3<f32>(tex_coord_and_derivatives.tex_coord, 0.0);
                } else if (constants.shading_mode == SHADING_MODE_EMISSION) {
                    color = material.emission;
                } else if (constants.shading_mode == SHADING_MODE_VELOCITY) {
                    let velocity: vec2<f32> = Gbuffer::load_velocity(id, view_index);
                    color = vec3<f32>(abs(velocity) * 10.0, 0.0);
                } else if (constants.shading_mode == SHADING_MODE_FOG) {
                    color = shade_fog(vec3<f32>(1.0), position_and_depth, ray.origin, ray.direction, l);
                }
            }
        } else {
            color = Sky::inscattering(ray.direction, false);
        }

        textureStore(color_out, id, view_index, vec4<f32>(color, emission));
    }
}