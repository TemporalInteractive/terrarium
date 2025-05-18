use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::wgpu_util::{ComputePipelineDescriptorExtensions, PipelineDatabase};

pub fn create_indirect_args_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrarium::write_indirect_args indirect_args"),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::INDIRECT,
        size: size_of::<u32>() as u64 * 3,
        mapped_at_creation: false,
    })
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    group_size: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

pub struct WriteIndirectArgsPassParameters<'a> {
    pub group_size: u32,
    pub count_buffer: &'a wgpu::Buffer,
    pub indirect_args_buffer: &'a wgpu::Buffer,
}

pub fn encode(
    parameters: &WriteIndirectArgsPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_wgsl!("../../shaders/write_indirect_args_pass.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::write_indirect_args"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::write_indirect_args"),
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
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
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
        label: Some("terrarium::write_indirect_args constants"),
        contents: bytemuck::bytes_of(&Constants {
            group_size: parameters.group_size,
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
                resource: parameters.count_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.indirect_args_buffer.as_entire_binding(),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::write_indirect_args"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("terrarium::write_indirect_args");
        cpass.dispatch_workgroups(1, 1, 1);
    }
}
