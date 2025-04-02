use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use wgpu::util::DeviceExt;

#[derive(Debug, Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct SunInfo {
    // Normalized sun direction
    pub direction: Vec3,
    // Radius in angular radians scaled by a magnitude of 10
    pub size: f32,
    // Artistic color, is used as normalized
    pub color: Vec3,
    // Intensity factor
    pub intensity: f32,
}

pub struct Sky {
    bind_group_layout: wgpu::BindGroupLayout,
    pub sun_info: SunInfo,
}

impl Default for SunInfo {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.2, -1.0, 0.3).normalize(),
            color: Vec3::new(1.0, 1.0, 1.0),
            size: 0.0,
            intensity: 100.0,
        }
    }
}

impl Sky {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        Self {
            bind_group_layout,
            sun_info: SunInfo::default(),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrarium::sky constants"),
            contents: bytemuck::bytes_of(&self.sun_info),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: constants.as_entire_binding(),
            }],
        })
    }
}
