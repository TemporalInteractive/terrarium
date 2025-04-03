@include terrarium/shaders/shared/material_pool.wgsl
@include terrarium/shaders/shared/math.wgsl

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = sqr(roughness);
    let a2 = sqr(a);
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = sqr(n_dot_h);
    
    let num = a2;
    let denom = PI * (n_dot_h2 * (a2 - 1.0) + 1.0) * (n_dot_h2 * (a2 - 1.0) + 1.0);
    
    return num / denom;
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = sqr(r) / 8.0;

    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
    
    return ggx1 * ggx2;
}

fn Material::eval_brdf(_self: Material, w_in_ws: vec3<f32>, w_out_ws: vec3<f32>, normal_ws: vec3<f32>) -> vec3<f32> {
    let h: vec3<f32> = normalize(w_in_ws + w_out_ws);
    let f0: vec3<f32> = mix(vec3<f32>(0.04), _self.color, _self.metallic);

    let NDF: f32 = distribution_ggx(normal_ws, h, _self.roughness);        
    let G: f32 = geometry_smith(normal_ws, w_out_ws, w_in_ws, _self.roughness);      
    let F: vec3<f32> = fresnel_schlick(max(dot(h, w_out_ws), 0.0), f0);       
    
    let kS: vec3<f32> = F;
    var kD: vec3<f32> = vec3(1.0) - kS;
    kD *= 1.0 - _self.metallic;	  
    
    let numerator: vec3<f32> = NDF * G * F;
    let denominator: f32 = 4.0 * max(dot(normal_ws, w_out_ws), 0.0) * max(dot(normal_ws, w_in_ws), 0.0) + 0.0001;
    let specular: vec3<f32> = numerator / denominator;

    return kD * _self.color / PI + specular;
}