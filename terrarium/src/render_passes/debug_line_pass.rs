use std::num::NonZeroU32;

use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::{debug_lines::DebugLines, GpuResources},
    wgpu_util::PipelineDatabase,
};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    _padding0: u32,
    _padding1: u32,
}

pub struct DebugLinePassParameters<'a> {
    pub resolution: UVec2,
    pub gpu_resources: &'a GpuResources,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub dst_view: &'a wgpu::TextureView,
    pub target_format: wgpu::TextureFormat,
}

pub fn encode(
    parameters: &DebugLinePassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let vertex_count = parameters.gpu_resources.debug_lines().vertex_count();
    if vertex_count > 0 {
        let shader = pipeline_database
            .shader_from_src(device, include_wgsl!("../../shaders/debug_line_pass.wgsl"));
        let pipeline = pipeline_database.render_pipeline(
            device,
            wgpu::RenderPipelineDescriptor {
                label: Some(&format!(
                    "terrarium::debug_line_pass {:?}",
                    parameters.target_format
                )),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[DebugLines::VERTEX_BUFFER_LAYOUT],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(parameters.target_format.into())],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Line,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: Some(NonZeroU32::new(2).unwrap()),
                cache: None,
            },
            || {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("terrarium::debug_line_pass"),
                    bind_group_layouts: &[&device.create_bind_group_layout(
                        &wgpu::BindGroupLayoutDescriptor {
                            label: None,
                            entries: &[
                                wgpu::BindGroupLayoutEntry {
                                    binding: 0,
                                    visibility: wgpu::ShaderStages::VERTEX,
                                    ty: wgpu::BindingType::Buffer {
                                        ty: wgpu::BufferBindingType::Uniform,
                                        has_dynamic_offset: false,
                                        min_binding_size: None,
                                    },
                                    count: None,
                                },
                                wgpu::BindGroupLayoutEntry {
                                    binding: 1,
                                    visibility: wgpu::ShaderStages::VERTEX,
                                    ty: wgpu::BindingType::Buffer {
                                        ty: wgpu::BufferBindingType::Uniform,
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
            label: Some("terrarium::gizmo constants"),
            contents: bytemuck::bytes_of(&Constants {
                resolution: parameters.resolution,
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
            ],
        });

        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: parameters.dst_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);

        rpass.set_bind_group(0, &bind_group, &[]);

        rpass.set_vertex_buffer(
            0,
            parameters
                .gpu_resources
                .debug_lines()
                .vertex_buffer()
                .slice(..),
        );

        rpass.draw(0..vertex_count, 0..1);
    }
}
