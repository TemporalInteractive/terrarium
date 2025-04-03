@include terrarium/shaders/shared/color.wgsl

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
var color: texture_storage_2d_array<rgba32float, read_write>;

fn hdr_to_sdr(hdr: vec3<f32>) -> vec3<f32> {
    let a: f32 = 2.51;
    let b: f32 = 0.03;
    let c: f32 = 2.43;
    let d: f32 = 0.59;
    let e: f32 = 0.14;
    
    let sdr: vec3<f32> = (hdr * (a * hdr + b)) / (hdr * (c * hdr + d) + e);
    return clamp(sdr, vec3<f32>(0.0), vec3<f32>(1.0));
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }

    for (var view_index: u32 = 0; view_index < 2; view_index += 1) {
        var hdr: vec3<f32> = textureLoad(color, id, view_index).rgb;

        var sdr: vec3<f32> = hdr_to_sdr(hdr);
        sdr = pow(sdr, vec3<f32>(1.0 / 2.2));

        textureStore(color, id, view_index, vec4<f32>(sdr, 1.0));
    }
}