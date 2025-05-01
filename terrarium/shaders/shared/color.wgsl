fn linear_to_srgb(linear: vec4<f32>) -> vec4<f32> {
    let cutoff = linear.rgb < vec3<f32>(0.0031308);
    let higher = vec3<f32>(1.055) * pow(linear.rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    let lower = linear.rgb * vec3<f32>(12.92);
    
    return vec4<f32>(select(higher, lower, cutoff), linear.a);
}

fn srgb_to_linear(srgb: vec4<f32>) -> vec4<f32> {
    let cutoff = srgb.rgb < vec3<f32>(0.04045);
    let higher = pow((srgb.rgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    let lower = srgb.rgb / vec3<f32>(12.92);
    
    return vec4<f32>(select(higher, lower, cutoff), srgb.a);
}

fn linear_to_luma(linear: vec3<f32>) -> f32 {
    return dot(linear, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn linear_to_ycbcr(rgb: vec3<f32>) -> vec3<f32> {
    let y  =  0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b;
    let cb = -0.1146 * rgb.r - 0.3854 * rgb.g + 0.5000 * rgb.b;
    let cr =  0.5000 * rgb.r - 0.4542 * rgb.g - 0.0458 * rgb.b;
    return vec3<f32>(y, cb, cr);
}

fn ycbcr_to_linear(ycbcr: vec3<f32>) -> vec3<f32> {
    let y  = ycbcr.x;
    let cb = ycbcr.y;
    let cr = ycbcr.z;

    let r = y + 1.5748 * cr;
    let g = y - 0.1873 * cb - 0.4681 * cr;
    let b = y + 1.8556 * cb;

    return vec3<f32>(r, g, b);
}