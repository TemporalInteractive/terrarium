@include shared/brdf.wgsl
@include shared/gbuffer.wgsl
@include shared/xr.wgsl

@include shared/vertex_pool_bindings.wgsl
@include shared/material_pool_bindings.wgsl
@include shared/sky_bindings.wgsl

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
var scene: acceleration_structure;

@group(0)
@binding(3)
var<storage, read> gbuffer_left: array<PackedGBufferTexel>;
@group(0)
@binding(4)
var<storage, read> gbuffer_right: array<PackedGBufferTexel>;

@group(0)
@binding(5)
var shadow: texture_2d_array<f32>;
@group(0)
@binding(6)
var shadow_sampler: sampler;

@group(0)
@binding(7)
var color_out: texture_storage_2d_array<rgba16float, read_write>;

fn shade_fog(shade_color: vec3<f32>, gbuffer_texel: GBufferTexel, view_dir: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    let density: f32 = sky_constants.atmosphere.density * Sky::atmosphere_density(gbuffer_texel.position_ws);
    let fog_strength: f32 = 1.0 - exp(-gbuffer_texel.depth_ws * density);
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
        var gbuffer_texel: GBufferTexel;
        if (view_index == 0) {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_left[i]);
        } else {
            gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_right[i]);
        }

        let ray: XrCameraRay = XrCamera::raygen(xr_camera, id, constants.resolution, view_index);

        var color: vec3<f32>;
        if (!GBufferTexel::is_sky(gbuffer_texel)) {
            let material_descriptor: MaterialDescriptor = material_descriptors[gbuffer_texel.material_descriptor_idx];
            var material: Material = Material::from_material_descriptor(material_descriptor, gbuffer_texel.tex_coord, gbuffer_texel.ddx, gbuffer_texel.ddy);

            let geometric_roughness: f32 = safe_sqrt(1.0 - gbuffer_texel.normal_roughness);
            material.roughness = safe_sqrt(sqr(material.roughness) + sqr(geometric_roughness));

            if (constants.shading_mode == SHADING_MODE_SIMPLE_LIGHTING) {
                let l: vec3<f32> = sky_constants.world_up;
                let n_dot_l: f32 = max(dot(gbuffer_texel.normal_ws, l), 0.0);

                let light_intensity: f32 = Sky::sun_intensity(l);
                let reflectance: vec3<f32> = Material::eval_brdf(material, l, -ray.direction, gbuffer_texel.normal_ws);

                let ambient: vec3<f32> = material.color * 0.1;

                color = reflectance * light_intensity * n_dot_l + ambient + material.emission;
            } else {
                let shadow: f32 = 1.0 - textureSampleLevel(shadow, shadow_sampler, (vec2<f32>(id) + vec2<f32>(0.5)) / vec2<f32>(constants.resolution), view_index, 0.0).r;

                let l: vec3<f32> = -sky_constants.sun.direction;
                let n_dot_l: f32 = max(dot(gbuffer_texel.normal_ws, l), 0.0);

                let light_intensity: f32 = shadow * Sky::sun_intensity(l);
                let reflectance: vec3<f32> = Material::eval_brdf(material, l, -ray.direction, gbuffer_texel.normal_ws);

                let ambient: vec3<f32> = material.color * 0.1;

                if (constants.shading_mode == SHADING_MODE_FULL) {
                    color = reflectance * light_intensity * n_dot_l + ambient + material.emission;
                    color = shade_fog(color, gbuffer_texel, ray.direction, l);
                } else if (constants.shading_mode == SHADING_MODE_LIGHTING_ONLY) {
                    color = vec3<f32>(light_intensity * n_dot_l) + ambient;
                } else if (constants.shading_mode == SHADING_MODE_ALBEDO) {
                    color = material.color;
                } else if (constants.shading_mode == SHADING_MODE_NORMALS) {
                    color = gbuffer_texel.normal_ws * 0.5 + 0.5;
                } else if (constants.shading_mode == SHADING_MODE_TEX_COORDS) {
                    color = vec3<f32>(gbuffer_texel.tex_coord, 0.0);
                } else if (constants.shading_mode == SHADING_MODE_EMISSION) {
                    color = material.emission;
                } else if (constants.shading_mode == SHADING_MODE_VELOCITY) {
                    color = vec3<f32>(abs(gbuffer_texel.velocity) * 10.0, 0.0);
                } else if (constants.shading_mode == SHADING_MODE_FOG) {
                    color = shade_fog(vec3<f32>(1.0), gbuffer_texel, ray.direction, l);
                }
            }
        } else {
            color = Sky::inscattering(ray.direction, false);
        }

        textureStore(color_out, id, view_index, vec4<f32>(color, 1.0));
    }
}