@include sampling.wgsl

const SUN_DISTANCE: f32 = 1e+6;

struct SunInfo {
    direction: vec3<f32>,
    size: f32,
    color: vec3<f32>,   
    intensity: f32,
}

struct AtmosphereInfo {
    inscattering_color: vec3<f32>,
    density: f32,
    density_noise_scale: f32,
    density_noise_min: f32,
    density_noise_max: f32,
    _padding0: u32,
}

struct SkyConstants {
    sun: SunInfo,
    atmosphere: AtmosphereInfo,
    world_up: vec3<f32>,
    _padding0: u32,
}

@group(3)
@binding(0)
var<uniform> sky_constants: SkyConstants;

fn Sky::sun_intensity(direction: vec3<f32>) -> f32 {
    return sky_constants.sun.intensity;
}

fn Sky::direction_to_sun(uv: vec2<f32>) -> vec3<f32> {
    return normalize(perturb_direction_vector(uv, -sky_constants.sun.direction, sky_constants.sun.size * 0.1));
}

fn Sky::sun_solid_angle() -> f32 {
    return TWO_PI * (1.0 - cos(sky_constants.sun.size * 0.1));
}

fn Sky::inscattering(direction: vec3<f32>, skip_sun: bool) -> vec3<f32> {
    const CUTOFF_ANGLE: f32 = PI / 1.95;
    let zenith_angle_cos: f32 = dot(-sky_constants.sun.direction, sky_constants.world_up);
    let intensity: f32 = max((clamp(zenith_angle_cos + 1.0, 0.0, 1.0) - 0.9) / 0.1, 0.0);
    
    let view_dir_dot_l: f32 = max(dot(direction, -sky_constants.sun.direction), 0.0);
    var inscattering: vec3<f32> = mix(sky_constants.atmosphere.inscattering_color, vec3<f32>(1.0, 0.9, 0.7), pow(view_dir_dot_l, 8.0) * intensity);

    if (!skip_sun) {
        var intensity = Sky::sun_intensity(direction);

        let l: vec3<f32> = -sky_constants.sun.direction;
        let cos_theta: f32 = dot(direction, l);
        let sun_angular_diameter_cos: f32 = cos(max(sky_constants.sun.size, 0.05) * 0.1);
        let sundisk: f32 = select(0.0, 1.0, cos_theta > sun_angular_diameter_cos);

       inscattering += intensity * 100.0 * sundisk * sky_constants.sun.color;
    }

    return inscattering;
}

fn random3(c: vec3<f32>) -> vec3<f32> {
    var j = 4096.0 * sin(dot(c, vec3<f32>(17.0, 59.4, 15.0)));
    var r: vec3<f32>;
    r.z = fract(512.0 * j);
    j *= 0.125;
    r.x = fract(512.0 * j);
    j *= 0.125;
    r.y = fract(512.0 * j);
    return r - 0.5;
}

fn simplex3d(p: vec3<f32>) -> f32 {
    const F3: f32 = 0.3333333;
    const G3: f32 = 0.1666667;

    let s = floor(p + dot(p, vec3<f32>(F3)));
    let x = p - s + dot(s, vec3<f32>(G3));

    let e = step(vec3<f32>(0.0), x - x.yzx);
    let i1 = e * (1.0 - e.zxy);
    let i2 = 1.0 - e.zxy * (1.0 - e);

    let x1 = x - i1 + vec3<f32>(G3);
    let x2 = x - i2 + vec3<f32>(2.0 * G3);
    let x3 = x - vec3<f32>(1.0) + vec3<f32>(3.0 * G3);

    var w: vec4<f32>;
    var d: vec4<f32>;

    w.x = dot(x, x);
    w.y = dot(x1, x1);
    w.z = dot(x2, x2);
    w.w = dot(x3, x3);

    w = max(vec4<f32>(0.6) - w, vec4<f32>(0.0));

    d.x = dot(random3(s), x);
    d.y = dot(random3(s + i1), x1);
    d.z = dot(random3(s + i2), x2);
    d.w = dot(random3(s + vec3<f32>(1.0)), x3);

    w = w * w;
    w = w * w;
    d = d * w;

    return dot(d, vec4<f32>(52.0));
}

fn Sky::atmosphere_density(view_origin: vec3<f32>, hit_point_ws: vec3<f32>) -> f32 {
    let noise_scale: f32 = sky_constants.atmosphere.density_noise_scale / 100.0;

    let noise_ws: f32 = simplex3d(noise_scale * hit_point_ws) * 0.5 + 0.5;
    let density_ws: f32 = mix(sky_constants.atmosphere.density_noise_min, sky_constants.atmosphere.density_noise_max, noise_ws);

    let noise_vs: f32 = simplex3d(noise_scale * 0.3 * view_origin) * 0.5 + 0.5;
    let density_vs: f32 = mix(0.5, 1.0, noise_vs);
    
    return density_ws * density_vs;
}