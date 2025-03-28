use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use terrarium::{
    app_loop::{AppLoop, AppLoopHandler, AppLoopHandlerCreateDesc},
    gpu_resources::{GpuMesh, GpuResources},
    helpers::input_handler::InputHandler,
    wgpu_util,
    world::{components::MeshComponent, transform::Transform},
    xr::XrHand,
    RenderParameters, Renderer,
};
use ugm::speedy::Readable;
use winit::window::Window;
use world::World;

mod world;

pub struct ExampleApp {
    input_handler: InputHandler,
    world: World,
    renderer: Renderer,
    gpu_resources: GpuResources,
}

impl AppLoop for ExampleApp {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        ctx: &wgpu_util::Context,
        _window: Arc<Window>,
    ) -> Self {
        let input_handler = InputHandler::new(&ctx.xr);
        let mut world = World::new();

        let renderer = Renderer::new(config, ctx);
        let mut gpu_resources = GpuResources::new(&ctx.device);

        let model =
            ugm::Model::read_from_buffer(&std::fs::read("examples/assets/Sponza.ugm").unwrap())
                .unwrap();
        world.create_entity("Sponza", Transform::default(), |builder| {
            let gpu_mesh = GpuMesh::new(&model.meshes[0], &mut gpu_resources, ctx);
            builder.with(MeshComponent::new(gpu_mesh))
        });

        Self {
            input_handler,
            world,
            renderer,
            gpu_resources,
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

        self.renderer.render(
            &mut RenderParameters {
                world: self.world.specs(),
                xr_camera_buffer,
                view,
                gpu_resources: &mut self.gpu_resources,
            },
            &mut command_encoder,
            ctx,
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
        self.renderer.resize(config, ctx);
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
            max_compute_invocations_per_workgroup: 512,
            max_compute_workgroup_size_x: 512,
            max_buffer_size: (1024 << 20),
            max_storage_buffer_binding_size: (1024 << 20),
            max_sampled_textures_per_shader_stage: 1024 * 32,
            max_push_constant_size: 128,
            max_bind_groups: 8,
            max_texture_dimension_1d: 4096,
            max_texture_dimension_2d: 4096,
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
