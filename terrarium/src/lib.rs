use glam::Vec3;

// Right handed coordinate system
pub const RIGHT: Vec3 = Vec3::new(1.0, 0.0, 0.0);
pub const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
pub const FORWARD: Vec3 = Vec3::new(0.0, 0.0, -1.0);

pub mod app_loop;
pub mod render_passes;
pub mod wgpu_util;
pub mod xr;
