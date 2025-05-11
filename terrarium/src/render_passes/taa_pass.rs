use std::sync::OnceLock;

use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec2};
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::gbuffer::Gbuffer,
    wgpu_util::{
        empty_bind_group, empty_bind_group_layout, ComputePipelineDescriptorExtensions,
        PipelineDatabase,
    },
};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    history_influence: f32,
    _padding0: u32,
}

pub struct TaaPassParameters<'a> {
    pub resolution: UVec2,
    pub history_influence: f32,
    pub color_texture_view: &'a wgpu::TextureView,
    pub prev_color_texture_view: &'a wgpu::TextureView,
    pub gbuffer: &'a Gbuffer,
    pub xr_camera_buffer: &'a wgpu::Buffer,
}

pub struct TaaJitter {
    samples: Vec<Vec2>,
}

impl TaaJitter {
    const SAMPLE_COUNT: u32 = 128;

    pub fn frame_jitter(frame_idx: u32) -> Vec2 {
        Self::get().samples[(frame_idx % Self::SAMPLE_COUNT) as usize]
    }

    fn get() -> &'static Self {
        static INSTANCE: OnceLock<TaaJitter> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let samples: Vec<glam::Vec2> = (0..Self::SAMPLE_COUNT)
                .map(|i| {
                    glam::Vec2::new(
                        Self::radical_inverse(i % Self::SAMPLE_COUNT + 1, 2) - 0.5,
                        Self::radical_inverse(i % Self::SAMPLE_COUNT + 1, 3) - 0.5,
                    )
                })
                .collect();

            Self { samples }
        })
    }

    fn radical_inverse(mut n: u32, base: u32) -> f32 {
        let mut val = 0.0f32;
        let inv_base = 1.0f32 / base as f32;
        let mut inv_bi = inv_base;

        while n > 0 {
            let d_i = n % base;
            val += d_i as f32 * inv_bi;
            n = (n as f32 * inv_base) as u32;
            inv_bi *= inv_base;
        }

        val
    }
}

pub fn encode(
    parameters: &TaaPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader =
        pipeline_database.shader_from_src(device, include_wgsl!("../../shaders/taa_pass.wgsl"));
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::taa"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::taa"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::Rgba16Float,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
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
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    }),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    parameters.gbuffer.bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::taa constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            history_influence: parameters.history_influence,
            _padding0: 0,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: constants.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(parameters.color_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(parameters.prev_color_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: parameters.xr_camera_buffer.as_entire_binding(),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::taa"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_bind_group(1, empty_bind_group(device), &[]);
        cpass.set_bind_group(2, empty_bind_group(device), &[]);
        cpass.set_bind_group(3, empty_bind_group(device), &[]);
        cpass.set_bind_group(4, parameters.gbuffer.bind_group(), &[]);
        cpass.insert_debug_marker("terrarium::taa");
        cpass.dispatch_workgroups(
            parameters.resolution.x.div_ceil(8),
            parameters.resolution.y.div_ceil(8),
            1,
        );
    }
}
