use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::GpuResources,
    wgpu_util::{ComputePipelineDescriptorExtensions, PipelineDatabase},
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ShadingMode {
    #[default]
    Full,
    LightingOnly,
    Albedo,
    Normals,
    Texcoords,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    shading_mode: u32,
    _padding0: u32,
}

pub struct ShadePassParameters<'a> {
    pub resolution: UVec2,
    pub shading_mode: ShadingMode,
    pub gpu_resources: &'a GpuResources,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub gbuffer: &'a [wgpu::Buffer; 2],
    pub shadow_texture_view: &'a wgpu::TextureView,
    pub dst_view: &'a wgpu::TextureView,
}

pub fn encode(
    parameters: &ShadePassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader =
        pipeline_database.shader_from_src(device, include_wgsl!("../../shaders/shade_pass.wgsl"));
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::shade"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::shade"),
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
                                ty: wgpu::BindingType::AccelerationStructure,
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
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 5,
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
                                binding: 6,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 7,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::Rgba16Float,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                },
                                count: None,
                            },
                        ],
                    }),
                    parameters.gpu_resources.vertex_pool().bind_group_layout(),
                    parameters.gpu_resources.material_pool().bind_group_layout(),
                    parameters.gpu_resources.sky().bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::shade constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            shading_mode: parameters.shading_mode as u32,
            _padding0: 0,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
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
                resource: parameters.xr_camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::AccelerationStructure(
                    parameters.gpu_resources.tlas(),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: parameters.gbuffer[0].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: parameters.gbuffer[1].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(parameters.shadow_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(parameters.dst_view),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::shade"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_bind_group(
            1,
            &parameters.gpu_resources.vertex_pool().bind_group(device),
            &[],
        );
        parameters.gpu_resources.material_pool().bind_group(
            pipeline.get_bind_group_layout(2),
            device,
            |bind_group| {
                cpass.set_bind_group(2, bind_group, &[]);
            },
        );
        cpass.set_bind_group(3, &parameters.gpu_resources.sky().bind_group(device), &[]);
        cpass.insert_debug_marker("terrarium::shade");
        cpass.dispatch_workgroups(
            parameters.resolution.x.div_ceil(16),
            parameters.resolution.y.div_ceil(16),
            1,
        );
    }
}
