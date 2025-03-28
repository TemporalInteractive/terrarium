use std::sync::Arc;
use winit::window::Window;

use crate::wgpu_util;

pub mod handler;

pub trait AppLoop: 'static + Sized {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        ctx: &wgpu_util::Context,
        window: Arc<Window>,
    ) -> Self;

    fn render(
        &mut self,
        xr_camera_buffer: &wgpu::Buffer,
        view: &wgpu::TextureView,
        ctx: &wgpu_util::Context,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) -> wgpu::CommandEncoder;
    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context);

    fn window_event(&mut self, _event: winit::event::WindowEvent) {}
    fn device_event(&mut self, _event: winit::event::DeviceEvent) {}

    fn optional_features() -> wgpu::Features {
        wgpu::Features::empty()
    }
    fn required_features() -> wgpu::Features {
        wgpu::Features::empty()
    }
    fn required_downlevel_capabilities() -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: wgpu::DownlevelFlags::empty(),
            shader_model: wgpu::ShaderModel::Sm5,
            ..wgpu::DownlevelCapabilities::default()
        }
    }
    fn required_limits() -> wgpu::Limits {
        wgpu::Limits::downlevel_webgl2_defaults()
    }
}
