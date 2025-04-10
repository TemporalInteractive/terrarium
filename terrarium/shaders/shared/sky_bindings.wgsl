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

       inscattering += intensity * 10000.0 * sundisk * sky_constants.sun.color;
    }

    return inscattering;
}