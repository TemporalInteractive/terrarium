use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::{gbuffer::Gbuffer, GpuResources},
    wgpu_util::{ComputePipelineDescriptorExtensions, PipelineDatabase},
};

use super::build_frustum_pass;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    lighting_resolution: UVec2,
    shadows: u32,
    shadow_bias: f32,
    _padding0: u32,
    _padding1: u32,
}

pub struct LtcLightingPassParameters<'a> {
    pub resolution: UVec2,
    pub lighting_resolution: UVec2,
    pub shadows: bool,
    pub shadow_bias: f32,
    pub gpu_resources: &'a GpuResources,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub gbuffer: &'a Gbuffer,
    pub ltc_instance_index_buffer: &'a wgpu::Buffer,
    pub ltc_instance_grid_texture_view: &'a wgpu::TextureView,
    pub dst_view: &'a wgpu::TextureView,
}

pub fn encode(
    parameters: &LtcLightingPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_wgsl!("../../shaders/ltc_lighting_pass.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::ltc_lighting"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::ltc_lighting"),
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
                                ty: wgpu::BindingType::AccelerationStructure {
                                    vertex_return: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::AccelerationStructure {
                                    vertex_return: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 4,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::Rgba16Float,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 5,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 6,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadOnly,
                                    format: wgpu::TextureFormat::Rg32Uint,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                },
                                count: None,
                            },
                        ],
                    }),
                    parameters.gpu_resources.vertex_pool().bind_group_layout(),
                    parameters.gpu_resources.material_pool().bind_group_layout(),
                    parameters.gpu_resources.sky().bind_group_layout(),
                    parameters.gbuffer.bind_group_layout(),
                    parameters
                        .gpu_resources
                        .linear_transformed_cosines()
                        .bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::ltc_lighting constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            lighting_resolution: parameters.lighting_resolution,
            shadows: parameters.shadows as u32,
            shadow_bias: parameters.shadow_bias,
            _padding0: 0,
            _padding1: 0,
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
                resource: wgpu::BindingResource::AccelerationStructure(
                    parameters.gpu_resources.static_tlas(),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::AccelerationStructure(
                    parameters.gpu_resources.dynamic_tlas(),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(parameters.dst_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: parameters.ltc_instance_index_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(
                    parameters.ltc_instance_grid_texture_view,
                ),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::ltc_lighting"),
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
        cpass.set_bind_group(4, parameters.gbuffer.bind_group(), &[]);
        cpass.set_bind_group(
            5,
            parameters
                .gpu_resources
                .linear_transformed_cosines()
                .bind_group(),
            &[],
        );
        cpass.insert_debug_marker("terrarium::ltc_lighting");
        cpass.dispatch_workgroups(
            parameters
                .lighting_resolution
                .x
                .div_ceil(build_frustum_pass::TILE_SIZE),
            parameters
                .lighting_resolution
                .y
                .div_ceil(build_frustum_pass::TILE_SIZE),
            1,
        );
    }
}
