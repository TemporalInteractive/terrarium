use glam::Mat4;
use wgsl_includes::include_wgsl;

use crate::wgpu_util::PipelineDatabase;

pub struct TriangleTestPassParameters<'a> {
    pub view_proj: Mat4,
    pub dst_view: &'a wgpu::TextureView,
    pub target_format: wgpu::TextureFormat,
}

pub fn encode(
    parameters: &TriangleTestPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_wgsl!("terrarium/shaders/triangle_test_pass.wgsl"),
    );
    let pipeline = pipeline_database.render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some("appearance-wgpu::blit"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(parameters.target_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("appearance-wgpu::blit"),
                bind_group_layouts: &[],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..std::mem::size_of::<Mat4>() as u32,
                }],
            })
        },
    );

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
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_push_constants(
            wgpu::ShaderStages::VERTEX,
            0,
            bytemuck::bytes_of(&parameters.view_proj),
        );
        rpass.draw(0..3, 0..1);
    }
}
