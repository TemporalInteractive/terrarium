use egui::epaint::Shadow;
use egui::{Context, Visuals};

use egui_winit::State;
use wgpu::{Device, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

mod renderer;
use renderer::Renderer;
pub use renderer::ScreenDescriptor;

use crate::wgpu_util;

pub struct EguiRenderer {
    context: Option<Context>,
    state: State,
    renderer: Renderer,
}

impl EguiRenderer {
    pub fn new(
        output_color_format: TextureFormat,
        msaa_samples: u32,
        window: &Window,
        device: &Device,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) -> EguiRenderer {
        let egui_context = Context::default();
        let id = egui_context.viewport_id();

        let visuals = Visuals {
            window_shadow: Shadow::NONE,
            ..Default::default()
        };

        egui_context.set_visuals(visuals);

        let egui_state =
            egui_winit::State::new(egui_context.clone(), id, &window, None, None, None);

        let egui_renderer =
            Renderer::new(output_color_format, msaa_samples, device, pipeline_database);

        EguiRenderer {
            context: Some(egui_context),
            state: egui_state,
            renderer: egui_renderer,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn begin_frame(&mut self, window: &Window) -> Context {
        let raw_input = self.state.take_egui_input(window);

        let context = self.context.take().expect("Frame was not ended.");
        context.begin_pass(raw_input);
        context
    }

    pub fn end_frame(&mut self, context: Context) {
        assert!(self.context.is_none());
        self.context = Some(context);
    }

    pub fn draw(
        &mut self,
        window: &Window,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        command_encoder: &mut wgpu::CommandEncoder,
    ) {
        let context = self.context.as_ref().expect("Frame was not ended.");
        // self.state.set_pixels_per_point(window.scale_factor() as f32);
        let full_output = context.end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = context.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        {
            self.renderer
                .update_buffers(device, queue, command_encoder, &tris, &screen_descriptor);

            let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: window_surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                label: Some("egui"),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.renderer.render(&mut rpass, &tris, &screen_descriptor);
        }

        for texture_id in &full_output.textures_delta.free {
            self.renderer.free_texture(texture_id);
        }
    }
}
