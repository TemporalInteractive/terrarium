use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::wgpu_util::{ComputePipelineDescriptorExtensions, PipelineDatabase};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    shadow_resolution: UVec2,
    seed: u32,
    sample_count: u32,
    radius: f32,
    intensity: f32,
    bias: f32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

pub struct SsaoPassParameters<'a> {
    pub resolution: UVec2,
    pub shadow_resolution: UVec2,
    pub seed: u32,
    pub sample_count: u32,
    pub radius: f32,
    pub intensity: f32,
    pub bias: f32,
    pub shadow_texture_view: &'a wgpu::TextureView,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub gbuffer: &'a [wgpu::Buffer; 2],
}

pub fn encode(
    parameters: &SsaoPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database
        .shader_from_src(device, include_wgsl!("../../shaders/ssao_pass.wgsl"));
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::ssao"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::ssao"),
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
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
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 4,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::R16Float,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                },
                                count: None,
                            },
                        ],
                    },
                )],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::ssao constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            shadow_resolution: parameters.shadow_resolution,
            seed: parameters.seed,
            sample_count: parameters.sample_count,
            radius: parameters.radius,
            intensity: parameters.intensity,
            bias: parameters.bias,
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
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
                resource: parameters.xr_camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.gbuffer[0].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: parameters.gbuffer[1].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(parameters.shadow_texture_view),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::ssao"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("terrarium::ssao");
        cpass.dispatch_workgroups(
            parameters.shadow_resolution.x.div_ceil(16),
            parameters.shadow_resolution.y.div_ceil(16),
            1,
        );
    }
}
