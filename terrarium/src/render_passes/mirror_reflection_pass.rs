use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::{gbuffer::Gbuffer, GpuResources},
    wgpu_util::{ComputePipelineDescriptorExtensions, PipelineDatabase},
};

use super::write_indirect_args_pass::{self, WriteIndirectArgsPassParameters};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    reflection_resolution: UVec2,
    ambient_factor: f32,
    view_index: u32,
    _padding0: u32,
    _padding1: u32,
}

pub struct MirrorReflectionPassParameters<'a> {
    pub resolution: UVec2,
    pub reflection_resolution: UVec2,
    pub ambient_factor: f32,
    pub gpu_resources: &'a GpuResources,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub gbuffer: &'a Gbuffer,
    pub reflection_counter_buffer: &'a [wgpu::Buffer; 2],
    pub reflection_pid_buffer: &'a [wgpu::Buffer; 2],
    pub dst_view: &'a wgpu::TextureView,
}

pub fn encode(
    parameters: &MirrorReflectionPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_wgsl!("../../shaders/mirror_reflection_pass.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::mirror_reflection"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::mirror_reflection"),
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
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    }),
                    parameters.gpu_resources.vertex_pool().bind_group_layout(),
                    parameters.gpu_resources.material_pool().bind_group_layout(),
                    parameters.gpu_resources.sky().bind_group_layout(),
                    parameters.gbuffer.bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    for view_index in 0..2 {
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrarium::mirror_reflection constants"),
            contents: bytemuck::bytes_of(&Constants {
                resolution: parameters.resolution,
                reflection_resolution: parameters.reflection_resolution,
                ambient_factor: parameters.ambient_factor,
                view_index,
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
                    resource: parameters.reflection_counter_buffer[view_index as usize]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: parameters.reflection_pid_buffer[view_index as usize]
                        .as_entire_binding(),
                },
            ],
        });

        let indirect_args_buffer = write_indirect_args_pass::create_indirect_args_buffer(device);
        write_indirect_args_pass::encode(
            &WriteIndirectArgsPassParameters {
                group_size: 128,
                count_buffer: &parameters.reflection_counter_buffer[view_index as usize],
                indirect_args_buffer: &indirect_args_buffer,
            },
            device,
            command_encoder,
            pipeline_database,
        );

        {
            let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("terrarium::mirror_reflection"),
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
            cpass.insert_debug_marker("terrarium::mirror_reflection");
            cpass.dispatch_workgroups_indirect(&indirect_args_buffer, 0);
        }
    }
}
