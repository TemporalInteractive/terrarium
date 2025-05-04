use std::num::NonZeroU32;

use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec2, Vec4};
use transform_gizmo::GizmoDrawData;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::wgpu_util::PipelineDatabase;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Vertex {
    position: Vec2,
    _padding0: u32,
    _padding1: u32,
    color: Vec4,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    _padding0: u32,
    _padding1: u32,
}

pub struct GizmoPassParameters<'a> {
    pub resolution: UVec2,
    pub gizmo_draw_data: &'a GizmoDrawData,
    pub dst_view: &'a wgpu::TextureView,
    pub target_format: wgpu::TextureFormat,
}

pub fn encode(
    parameters: &GizmoPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    if parameters.gizmo_draw_data.indices.is_empty() {
        return;
    }

    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                // Position
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                // Color
                format: wgpu::VertexFormat::Float32x4,
                offset: 4 * std::mem::size_of::<f32>() as u64,
                shader_location: 1,
            },
        ],
    };

    let color_target_state = Some(wgpu::ColorTargetState {
        format: parameters.target_format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    });

    let shader =
        pipeline_database.shader_from_src(device, include_wgsl!("../../shaders/gizmo_pass.wgsl"));
    let pipeline = pipeline_database.render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some(&format!("terrarium::gizmo {:?}", parameters.target_format)),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[color_target_state],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: Some(NonZeroU32::new(2).unwrap()), // TODO: this doesn't really make sense as the transform-gizmo crate already projects the vertices using only the first camera matrix
            cache: None,
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::gizmo"),
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
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
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: constants.as_entire_binding(),
        }],
    });

    let vertices: Vec<Vertex> = parameters
        .gizmo_draw_data
        .vertices
        .iter()
        .zip(parameters.gizmo_draw_data.colors.iter())
        .map(|(position, color)| Vertex {
            position: Vec2::from_array(*position),
            _padding0: 0,
            _padding1: 1,
            color: Vec4::from_array(*color),
        })
        .collect();

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::gizmo vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::gizmo indices"),
        contents: bytemuck::cast_slice(&parameters.gizmo_draw_data.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    {
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terrarium::gizmo"),
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

        rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
        rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        rpass.draw_indexed(0..parameters.gizmo_draw_data.indices.len() as u32, 0, 0..1);
    }
}
