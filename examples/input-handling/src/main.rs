use std::sync::Arc;

use clap::Parser;
use glam::Mat4;
use terrarium::{
    app_loop::{
        handler::{AppLoopHandler, AppLoopHandlerCreateDesc},
        AppLoop,
    },
    helpers::input_handler::InputHandler,
    render_passes::debug_pass::{self, DebugPassParameters},
    wgpu_util,
    xr::XrHand,
};

use anyhow::Result;
use ugm::speedy::Readable;
use wgpu::util::DeviceExt;
use winit::window::Window;

struct SizedResources {
    depth_texture: wgpu::Texture,
}

impl SizedResources {
    pub fn new(config: &wgpu::SurfaceConfiguration, device: &wgpu::Device) -> Self {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("example-input-handling::depth"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        Self { depth_texture }
    }
}

pub struct ExampleApp {
    input_handler: InputHandler,

    model: ugm::Model,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,

    sized_resources: SizedResources,
}

impl AppLoop for ExampleApp {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        ctx: &wgpu_util::Context,
        _window: Arc<Window>,
    ) -> Self {
        let input_handler = InputHandler::new(&ctx.xr);

        let model =
            ugm::Model::read_from_buffer(&std::fs::read("examples/assets/Sponza.ugm").unwrap())
                .unwrap();

        let vertex_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("example-input-handling::vertices"),
                contents: bytemuck::cast_slice(model.meshes[0].packed_vertices.as_slice()),
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST,
            });

        let index_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("example-input-handling::indices"),
                contents: bytemuck::cast_slice(model.meshes[0].indices.as_slice()),
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST,
            });

        let sized_resources = SizedResources::new(config, &ctx.device);

        Self {
            input_handler,
            model,
            vertex_buffer,
            index_buffer,
            sized_resources,
        }
    }

    fn render(
        &mut self,
        xr_camera_buffer: &wgpu::Buffer,
        view: &wgpu::TextureView,
        ctx: &wgpu_util::Context,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) -> wgpu::CommandEncoder {
        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        debug_pass::encode(
            &DebugPassParameters {
                local_to_world_space: Mat4::from_cols_array(&self.model.nodes[0].transform),
                xr_camera_buffer,
                dst_view: view,
                target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
                vertex_buffer: &self.vertex_buffer,
                index_buffer: &self.index_buffer,
                depth_texture: &self.sized_resources.depth_texture,
            },
            &ctx.device,
            &mut command_encoder,
            pipeline_database,
        );

        if let Some(thumbstick) = self
            .input_handler
            .current()
            .xr_hand(XrHand::Right)
            .analog_2d("/input/thumbstick")
        {
            println!("value: {}", thumbstick);
        }

        self.input_handler.update();

        command_encoder
    }

    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) {
        self.sized_resources = SizedResources::new(config, &ctx.device);
    }

    fn window_event(&mut self, event: winit::event::WindowEvent) {
        self.input_handler.handle_window_input(&event);
    }

    fn device_event(&mut self, event: winit::event::DeviceEvent) {
        self.input_handler.handle_device_input(&event);
    }

    fn xr_post_frame(&mut self, xr_frame_state: &openxr::FrameState, xr: &wgpu_util::XrContext) {
        self.input_handler.handle_xr_input(xr_frame_state, xr);
    }

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

    AppLoopHandler::<ExampleApp>::new(&AppLoopHandlerCreateDesc {
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
