use std::sync::Arc;

use anyhow::Result;
use camera_controller::CameraController;
use clap::Parser;
use glam::Mat4;
use terrarium::{
    app_loop::{AppLoop, AppLoopHandler, AppLoopHandlerCreateDesc},
    gpu_resources::{GpuMaterial, GpuMesh, GpuResources},
    helpers::{
        input_handler::InputHandler,
        timer::{FpsCounter, Timer},
    },
    wgpu_util,
    world::{components::MeshComponent, transform::Transform},
    xr::XrCameraState,
    RenderParameters, Renderer,
};
use ugm::{speedy::Readable, Model};
use winit::window::Window;
use world::World;

mod camera_controller;
mod world;

pub fn spawn_model(
    model: &Model,
    world: &mut World,
    gpu_resources: &mut GpuResources,
    command_encoder: &mut wgpu::CommandEncoder,
    ctx: &wgpu_util::Context,
) {
    let gpu_meshes: Vec<GpuMesh> = model
        .meshes
        .iter()
        .map(|mesh| gpu_resources.create_gpu_mesh(mesh, command_encoder, ctx))
        .collect();

    let gpu_materials: Vec<GpuMaterial> = model
        .materials
        .iter()
        .map(|material| gpu_resources.create_gpu_material(model, material, ctx))
        .collect();

    model.traverse_nodes(Mat4::IDENTITY, |node, transform| {
        if let Some(mesh_idx) = node.mesh_idx {
            world.create_entity(&node.name, Transform::from(transform), |builder| {
                builder.with(MeshComponent::new(
                    gpu_meshes[mesh_idx as usize].clone(),
                    vec![],
                ))
            });
        }
    });
}

pub struct ExampleApp {
    input_handler: InputHandler,
    world: World,
    camera_controller: CameraController,
    renderer: Renderer,
    aspect_ratio: f32,
    gpu_resources: GpuResources,
    frame_timer: Timer,
    fps_counter: FpsCounter,
    first_frame: bool,
}

impl AppLoop for ExampleApp {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        ctx: &wgpu_util::Context,
        _window: Arc<Window>,
    ) -> Self {
        let input_handler = InputHandler::new(&ctx.xr);
        let world = World::new();

        let renderer = Renderer::new(config, ctx);
        let gpu_resources = GpuResources::new(&ctx.device);

        let aspect_ratio = config.width as f32 / config.height as f32;

        Self {
            input_handler,
            world,
            camera_controller: CameraController::new(),
            renderer,
            aspect_ratio,
            gpu_resources,
            frame_timer: Timer::new(),
            fps_counter: FpsCounter::new(),
            first_frame: true,
        }
    }

    fn render(
        &mut self,
        xr_camera_state: &mut XrCameraState,
        xr_camera_buffer: &wgpu::Buffer,
        view: &wgpu::TextureView,
        ctx: &wgpu_util::Context,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) -> wgpu::CommandEncoder {
        let delta_time = self.frame_timer.elapsed();
        self.frame_timer.reset();

        self.camera_controller
            .update(&self.input_handler, delta_time, xr_camera_state);
        self.camera_controller
            .update_xr_camera_state(self.aspect_ratio, xr_camera_state);

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        if self.first_frame {
            self.first_frame = false;

            let model = ugm::Model::read_from_buffer(
                &std::fs::read("examples/massive/assets/TestSceneBig.ugm")
                .expect("It looks like you're missing the TestScene.glb model. Please download it from here https://drive.google.com/file/d/1Phta9UH7fvtCCOQMh3c0YxrL6kYzjcJc/view?usp=drive_link and place it in the assets folder."),
            )
            .unwrap();
            spawn_model(
                &model,
                &mut self.world,
                &mut self.gpu_resources,
                &mut command_encoder,
                ctx,
            );
        }

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

        self.input_handler.update();
        self.fps_counter.end_frame();

        println!("FPS {}", self.fps_counter.fps());

        command_encoder
    }

    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) {
        self.aspect_ratio = config.width as f32 / config.height as f32;

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
        wgpu::Features::MULTIVIEW
            | wgpu::Features::PUSH_CONSTANTS
            | wgpu::Features::TEXTURE_BINDING_ARRAY
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
            | wgpu::Features::EXPERIMENTAL_RAY_TRACING_ACCELERATION_STRUCTURE
            | wgpu::Features::EXPERIMENTAL_RAY_QUERY
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
