@include shared/brdf.wgsl
@include shared/gbuffer.wgsl
@include shared/xr.wgsl

@include shared/vertex_pool_bindings.wgsl
@include shared/material_pool_bindings.wgsl
@include shared/sky_bindings.wgsl

struct Constants {
    resolution: vec2<u32>,
    _padding0: u32,
    _padding1: u32,
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
var color_out: texture_storage_2d_array<rgba32float, read_write>;

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
            let material: Material = Material::from_material_descriptor(material_descriptor, gbuffer_texel.tex_coord);

            let shadow: f32 = textureSampleLevel(shadow, shadow_sampler, (vec2<f32>(id) + vec2<f32>(0.5)) / vec2<f32>(constants.resolution), view_index, 0.0).r;

            let l: vec3<f32> = Sky::direction_to_sun(vec2<f32>(0.5));
            let n_dot_l: f32 = max(dot(gbuffer_texel.normal_ws, l), 0.0);

            let light_intensity: f32 = shadow * Sky::sun_intensity(l);
            let reflectance: vec3<f32> = Material::eval_brdf(material, l, -ray.direction, gbuffer_texel.normal_ws);

            color = reflectance * max(light_intensity * n_dot_l, 0.2);
        } else {
            color = Sky::sky(ray.direction, false);
        }

        textureStore(color_out, id, view_index, vec4<f32>(color, 1.0));
    }
}