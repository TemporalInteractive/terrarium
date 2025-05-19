use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::{gbuffer::Gbuffer, GpuResources},
    wgpu_util::{
        empty_bind_group, empty_bind_group_layout, ComputePipelineDescriptorExtensions,
        PipelineDatabase,
    },
};

pub fn create_reflection_counter_buffer(_resolution: UVec2, device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrarium::rt_gbuffer reflection_counter"),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        size: size_of::<u32>() as u64,
        mapped_at_creation: false,
    })
}

pub fn create_reflection_pid_buffer(resolution: UVec2, device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrarium::rt_gbuffer reflection_pid"),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        size: (size_of::<u32>() as u32 * resolution.x * resolution.y * 2) as u64,
        mapped_at_creation: false,
    })
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    mipmapping: u32,
    normal_mapping: u32,
    reflection_max_roughness: f32,
    view_index: u32,
    render_distance: f32,
    _padding0: u32,
}

pub struct RtGbufferPassParameters<'a> {
    pub resolution: UVec2,
    pub mipmapping: bool,
    pub normal_mapping: bool,
    pub reflection_max_roughness: f32,
    pub render_distance: f32,
    pub gpu_resources: &'a GpuResources,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub gbuffer: &'a Gbuffer,
    pub reflection_counter_buffer: &'a [wgpu::Buffer; 2],
    pub reflection_pid_buffer: &'a [wgpu::Buffer; 2],
}

pub fn encode(
    parameters: &RtGbufferPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database
        .shader_from_src(device, include_wgsl!("../../shaders/rt_gbuffer_pass.wgsl"));
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::rt_gbuffer"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::rt_gbuffer"),
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
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 5,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    }),
                    parameters.gpu_resources.vertex_pool().bind_group_layout(),
                    parameters.gpu_resources.material_pool().bind_group_layout(),
                    empty_bind_group_layout(device),
                    parameters.gbuffer.bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    for view_index in 0..2 {
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrarium::rt_gbuffer constants"),
            contents: bytemuck::bytes_of(&Constants {
                resolution: parameters.resolution,
                mipmapping: parameters.mipmapping as u32,
                normal_mapping: parameters.normal_mapping as u32,
                reflection_max_roughness: parameters.reflection_max_roughness,
                view_index,
                render_distance: parameters.render_distance,
                _padding0: 0,
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
                    resource: parameters.reflection_counter_buffer[view_index as usize]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: parameters.reflection_pid_buffer[view_index as usize]
                        .as_entire_binding(),
                },
            ],
        });

        command_encoder.clear_buffer(
            &parameters.reflection_counter_buffer[view_index as usize],
            0,
            None,
        );

        {
            let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("terrarium::rt_gbuffer"),
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
            cpass.set_bind_group(3, empty_bind_group(device), &[]);
            cpass.set_bind_group(4, parameters.gbuffer.bind_group(), &[]);
            cpass.insert_debug_marker("terrarium::rt_gbuffer");
            cpass.dispatch_workgroups(
                parameters.resolution.x.div_ceil(8),
                parameters.resolution.y.div_ceil(8),
                1,
            );
        }
    }
}
