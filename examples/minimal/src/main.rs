use terrarium::{
    app_loop::{
        handler::{AppLoopHandler, AppLoopHandlerCreateDesc},
        AppLoop,
    },
    wgpu_util,
};

use anyhow::Result;
use winit::window::Window;

pub struct MinimalApp {}

impl AppLoop for MinimalApp {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        ctx: &std::sync::Arc<wgpu_util::Context>,
        window: std::sync::Arc<Window>,
    ) -> Self {
        Self {}
    }

    fn render(&mut self, view: &wgpu::TextureView, ctx: &wgpu_util::Context) -> bool {
        false
    }

    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) {}

    fn required_features() -> wgpu::Features {
        wgpu::Features::MULTIVIEW | wgpu::Features::PUSH_CONSTANTS
    }

    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_1d: 4096,
            max_texture_dimension_2d: 4096,
            ..wgpu::Limits::default()
        }
    }
}

fn main() -> Result<()> {
    AppLoopHandler::<MinimalApp>::new(&AppLoopHandlerCreateDesc {
        title: "Terrarium".to_owned(),
        width: 1920,
        height: 1080,
        resizeable: false,
        maximized: false,
        no_gpu_validation: false,
    })
    .run()?;

    Ok(())
}
