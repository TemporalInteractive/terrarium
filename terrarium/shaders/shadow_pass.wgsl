// @include shared/random.wgsl
// @include shared/trace.wgsl
// @include shared/xr.wgsl

// @include shared/sky_bindings.wgsl

// struct Constants {
//     resolution: vec2<u32>,
//     shadow_resolution: vec2<u32>,
//     seed: u32,
//     _padding0: u32,
//     _padding1: u32,
//     _padding2: u32,
// }

// @group(0)
// @binding(0)
// var<uniform> constants: Constants;

// @group(0)
// @binding(1)
// var<uniform> xr_camera: XrCamera;

// @group(0)
// @binding(2)
// var scene: acceleration_structure;

// @group(0)
// @binding(3)
// var<storage, read> gbuffer_left: array<PackedGBufferTexel>;
// @group(0)
// @binding(4)
// var<storage, read> gbuffer_right: array<PackedGBufferTexel>;

// @group(0)
// @binding(5)
// var shadow_out: texture_storage_2d_array<r16float, read_write>;

// @compute
// @workgroup_size(16, 16)
// fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
//     @builtin(num_workgroups) dispatch_size: vec3<u32>) {
//     let shadow_id: vec2<u32> = global_id.xy;
//     if (any(shadow_id >= constants.shadow_resolution)) { return; }

//     let id = vec2<u32>(
//         u32(f32(constants.resolution.x) / f32(constants.shadow_resolution.x) * f32(shadow_id.x)),
//         u32(f32(constants.resolution.y) / f32(constants.shadow_resolution.y) * f32(shadow_id.y))
//     );
//     let i: u32 = id.y * constants.resolution.x + id.x;

//     var rng: u32 = pcg_hash(i ^ xor_shift_u32(constants.seed));

//     for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
//         var gbuffer_texel: GBufferTexel;
//         if (view_index == 0) {
//             gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_left[i]);
//         } else {
//             gbuffer_texel = PackedGBufferTexel::unpack(gbuffer_right[i]);
//         }

//         let shadow_origin: vec3<f32> = gbuffer_texel.position_ws;
//         let shadow_direction: vec3<f32> = Sky::direction_to_sun(random_uniform_float2(&rng));

//         var shadow: f32 = 1.0;
//         if (dot(shadow_direction, gbuffer_texel.normal_ws) > 0.0 && trace_shadow_ray_opaque(shadow_origin, shadow_direction, 10000.0, normalize(gbuffer_texel.geometric_normal_ws), scene)) {
//             shadow = 0.0;
//         }

//         textureStore(shadow_out, shadow_id, view_index, vec4<f32>(vec3<f32>(shadow), 1.0));
//     }
// }