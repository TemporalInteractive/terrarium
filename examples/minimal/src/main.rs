use clap::Parser;
use glam::{Mat4, Vec3};
use terrarium::{
    app_loop::{
        handler::{AppLoopHandler, AppLoopHandlerCreateDesc},
        AppLoop,
    },
    render_passes::triangle_test_pass::{self, TriangleTestPassParameters},
    wgpu_util,
};

use anyhow::Result;
use winit::window::Window;

pub struct MinimalApp {
    swapchain_format: wgpu::TextureFormat,
    pipeline_database: wgpu_util::PipelineDatabase,
}

impl AppLoop for MinimalApp {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        _ctx: &std::sync::Arc<wgpu_util::Context>,
        _window: std::sync::Arc<Window>,
    ) -> Self {
        Self {
            swapchain_format: config.view_formats[0],
            pipeline_database: wgpu_util::PipelineDatabase::new(),
        }
    }

    fn render(&mut self, view: &wgpu::TextureView, ctx: &wgpu_util::Context) -> bool {
        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        triangle_test_pass::encode(
            &TriangleTestPassParameters {
                view_proj: Mat4::perspective_rh(60.0, 1.0, 0.01, 100.0)
                    * Mat4::from_translation(Vec3::new(0.0, 0.0, -1.0)),
                dst_view: view,
                target_format: self.swapchain_format,
            },
            &ctx.device,
            &mut command_encoder,
            &mut self.pipeline_database,
        );

        // TODO: move
        ctx.queue.submit(Some(command_encoder.finish()));

        false
    }

    fn resize(&mut self, _config: &wgpu::SurfaceConfiguration, _ctx: &wgpu_util::Context) {}

    fn required_features() -> wgpu::Features {
        wgpu::Features::MULTIVIEW | wgpu::Features::PUSH_CONSTANTS
    }

    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_1d: 4096,
            max_texture_dimension_2d: 4096,
            max_push_constant_size: 128,
            ..wgpu::Limits::default()
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Forcefully disable gpu validation
    #[arg(long, default_value_t = false)]
    no_gpu_validation: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    AppLoopHandler::<MinimalApp>::new(&AppLoopHandlerCreateDesc {
        title: "Terrarium".to_owned(),
        width: 1920,
        height: 1080,
        resizeable: false,
        maximized: false,
        no_gpu_validation: args.no_gpu_validation,
    })
    .run()?;

    Ok(())
}
