use std::num::NonZeroU32;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use specs::Join;
use ugm::mesh::PackedVertex;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::GpuResources,
    wgpu_util::PipelineDatabase,
    world::components::{MeshComponent, TransformComponent},
};

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct PushConstant {
    local_to_world_space: Mat4,
    inv_trans_local_to_world_space: Mat4,
}

pub struct GbufferPassParameters<'a> {
    pub world: &'a specs::World,
    pub gpu_resources: &'a GpuResources,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub dst_view: &'a wgpu::TextureView,
    pub target_format: wgpu::TextureFormat,
    pub depth_texture: &'a wgpu::Texture,
}

pub fn encode(
    parameters: &GbufferPassParameters,
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
        .shader_from_src(device, include_wgsl!("terrarium/shaders/gbuffer_pass.wgsl"));
    let pipeline = pipeline_database.render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some(&format!(
                "terrarium::gbuffer_pass {:?}",
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
                label: Some("terrarium::gbuffer_pass"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    }),
                    parameters.gpu_resources.vertex_pool().bind_group_layout(),
                ],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..size_of::<PushConstant>() as u32,
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
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.set_bind_group(
            1,
            &parameters.gpu_resources.vertex_pool().bind_group(device),
            &[],
        );

        let vertex_pool = parameters.gpu_resources.vertex_pool();
        rpass.set_vertex_buffer(0, vertex_pool.vertex_buffer().slice(..));
        rpass.set_index_buffer(
            vertex_pool.index_buffer().slice(..),
            wgpu::IndexFormat::Uint32,
        );

        let (transform_storage, mesh_storage): (
            specs::ReadStorage<'_, TransformComponent>,
            specs::ReadStorage<'_, MeshComponent>,
        ) = parameters.world.system_data();

        for (transform_component, mesh_component) in (&transform_storage, &mesh_storage).join() {
            let vertex_slice = &mesh_component.mesh.vertex_pool_alloc.slice;
            let local_to_world_space = transform_component.transform.get_matrix();
            let inv_trans_local_to_world_space = transform_component
                .transform
                .get_matrix()
                .inverse()
                .transpose();

            rpass.set_push_constants(
                wgpu::ShaderStages::VERTEX,
                0,
                bytemuck::bytes_of(&PushConstant {
                    local_to_world_space,
                    inv_trans_local_to_world_space,
                }),
            );
            rpass.draw_indexed(
                vertex_slice.first_index()..vertex_slice.last_index(),
                vertex_slice.first_vertex() as i32,
                0..1,
            );
        }
    }
}
