@include sampling.wgsl

const SUN_DISTANCE: f32 = 1e+6;

struct SkyConstants {
    sun_direction: vec3<f32>,
    sun_size: f32,
    sun_color: vec3<f32>,   
    sun_intensity: f32,
}

@group(3)
@binding(0)
var<uniform> sky_constants: SkyConstants;

fn Sky::sun_intensity(direction: vec3<f32>) -> f32 {
    const CUTOFF_ANGLE: f32 = PI / 1.95;

    let l: vec3<f32> = -sky_constants.sun_direction;
    let zenith_angle_cos: f32 = dot(l, UP);
    var intensity: f32 = sky_constants.sun_intensity *
        max(0.0, 1.0 - exp(-((CUTOFF_ANGLE - acos(zenith_angle_cos)) / 1.4)));

    return intensity;
}

fn Sky::direction_to_sun(uv: vec2<f32>) -> vec3<f32> {
    return normalize(perturb_direction_vector(uv, -sky_constants.sun_direction, sky_constants.sun_size * 0.1));
}

fn Sky::sun_solid_angle() -> f32 {
    return TWO_PI * (1.0 - cos(sky_constants.sun_size * 0.1));
}

fn Sky::sky(direction: vec3<f32>, skip_sun: bool) -> vec3<f32> {
    var sky_color = vec3<f32>(0.83, 0.8, 0.9);

    if (!skip_sun) {
        var intensity = Sky::sun_intensity(direction);

        let l: vec3<f32> = -sky_constants.sun_direction;
        let cos_theta: f32 = dot(direction, l);
        let sun_angular_diameter_cos: f32 = cos(max(sky_constants.sun_size, 0.1) * 0.1);
        let sundisk: f32 = select(0.0, 1.0, cos_theta > sun_angular_diameter_cos);

       sky_color += intensity * 10000.0 * sundisk * sky_constants.sun_color;
    }

    return sky_color;
}