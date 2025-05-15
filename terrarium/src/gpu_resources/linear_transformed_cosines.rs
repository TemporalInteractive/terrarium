use std::io::Cursor;

use bytemuck::{Pod, Zeroable};
use ddsfile::Dds;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

const MAX_INSTANCES: usize = 1024 * 128;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    instance_count: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct LtcInstance {
    transform: Mat4,
    inv_transform: Mat4,
    color: Vec3,
    double_sided: u32,
}

pub struct LinearTransformedCosines {
    constants_buffer: wgpu::Buffer,
    instances_buffer: wgpu::Buffer,
    instances: Vec<LtcInstance>,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl LinearTransformedCosines {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let create_texture = |bytes| -> wgpu::TextureView {
            let mut cursor = Cursor::new(&bytes);
            let dds = Dds::read(&mut cursor).unwrap();
            assert_eq!(dds.get_width(), 64);
            assert_eq!(dds.get_height(), 64);
            assert_eq!(dds.get_num_array_layers(), 1);

            let ltc1_texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("terrarium::linear_transformed_cosines lut"),
                    size: wgpu::Extent3d {
                        width: 64,
                        height: 64,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::wgt::TextureDataOrder::LayerMajor,
                &dds.data,
            );
            ltc1_texture.create_view(&wgpu::TextureViewDescriptor::default())
        };

        let ltc1_texture_view = create_texture(include_bytes!("../../assets/ltc_1.dds"));
        let ltc2_texture_view = create_texture(include_bytes!("../../assets/ltc_2.dds"));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let constants_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::linear_transformed_cosines constants"),
            size: size_of::<Constants>() as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let instances_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::linear_transformed_cosines instances"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<LtcInstance>() * MAX_INSTANCES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: constants_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&ltc1_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ltc2_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: instances_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            constants_buffer,
            instances_buffer,
            instances: Vec::new(),
            bind_group_layout,
            bind_group,
        }
    }

    pub fn write_instances(&mut self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.constants_buffer,
            0,
            bytemuck::bytes_of(&Constants {
                instance_count: self.instances.len() as u32,
                _padding0: 0,
                _padding1: 0,
                _padding2: 0,
            }),
        );

        queue.write_buffer(
            &self.instances_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );
    }

    pub fn submit_instance(&mut self, transform: Mat4, color: Vec3, double_sided: bool) {
        self.instances.push(LtcInstance {
            transform,
            inv_transform: transform.inverse(),
            color,
            double_sided: double_sided as u32,
        });
        assert!(self.instances.len() < MAX_INSTANCES);
    }

    pub fn end_frame(&mut self) {
        self.instances.clear();
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
