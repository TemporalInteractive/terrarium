use std::num::NonZeroU32;

use glam::Mat4;
use ugm::mesh::PackedVertex;
use wgsl_includes::include_wgsl;

use crate::wgpu_util::PipelineDatabase;

pub struct DebugPassParameters<'a> {
    pub view_proj: Mat4,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub dst_view: &'a wgpu::TextureView,
    pub target_format: wgpu::TextureFormat,
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub depth_texture: &'a wgpu::Texture,
}

pub fn encode(
    parameters: &DebugPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<PackedVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                // Position
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                // Normal
                format: wgpu::VertexFormat::Uint32,
                offset: 3 * std::mem::size_of::<f32>() as u64,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                // Texcoord
                format: wgpu::VertexFormat::Float32x2,
                offset: 4 * std::mem::size_of::<f32>() as u64,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                // Tangent
                format: wgpu::VertexFormat::Uint32,
                offset: 6 * std::mem::size_of::<f32>() as u64,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                // Tangent handiness
                format: wgpu::VertexFormat::Float32,
                offset: 7 * std::mem::size_of::<f32>() as u64,
                shader_location: 4,
            },
        ],
    };

    let depth_stencil = Some(wgpu::DepthStencilState {
        format: parameters.depth_texture.format(),
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    });

    let shader = pipeline_database
        .shader_from_src(device, include_wgsl!("terrarium/shaders/debug_pass.wgsl"));
    let pipeline = pipeline_database.render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some(&format!(
                "terrarium::debug_pass {:?}",
                parameters.target_format
            )),
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
                targets: &[Some(parameters.target_format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil,
            multisample: wgpu::MultisampleState::default(),
            multiview: Some(NonZeroU32::new(2).unwrap()),
            cache: None,
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::debug_pass"),
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
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..size_of::<Mat4>() as u32,
                }],
            })
        },
    );

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: parameters.xr_camera_buffer.as_entire_binding(),
        }],
    });

    let depth_view = parameters
        .depth_texture
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            ..Default::default()
        });

    {
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: parameters.dst_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_push_constants(
            wgpu::ShaderStages::VERTEX,
            0,
            bytemuck::bytes_of(&parameters.view_proj),
        );

        rpass.set_vertex_buffer(0, parameters.vertex_buffer.slice(..));
        rpass.set_index_buffer(parameters.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw_indexed(0..(parameters.index_buffer.size() as u32 / 4), 0, 0..1);
    }
}
