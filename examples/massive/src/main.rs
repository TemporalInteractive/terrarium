use std::sync::Arc;

use anyhow::Result;
use camera_controller::CameraController;
use clap::Parser;
use glam::{Quat, UVec2, Vec3};
use terrarium::{
    app_loop::{AppLoop, AppLoopHandler, AppLoopHandlerCreateDesc},
    egui,
    gpu_resources::GpuResources,
    helpers::{
        input_handler::InputHandler,
        timer::{FpsCounter, Timer},
    },
    wgpu_util,
    world::{
        components::TransformComponent,
        transform::{Transform, FORWARD, RIGHT, UP},
    },
    xr::XrCameraState,
    RenderParameters, RenderSettings, Renderer,
};
use ugm::speedy::Readable;
use winit::window::Window;
use world::World;

mod camera_controller;
mod world;

pub struct ExampleApp {
    input_handler: InputHandler,
    world: World,
    camera_controller: CameraController,
    renderer: Renderer,
    render_settings: RenderSettings,
    aspect_ratio: f32,
    gpu_resources: GpuResources,
    frame_timer: Timer,
    fps_counter: FpsCounter,
    first_frame: bool,

    emitter_entity: Option<specs::Entity>,
}

impl AppLoop for ExampleApp {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        ctx: &wgpu_util::Context,
        _window: Arc<Window>,
    ) -> Self {
        let input_handler = InputHandler::new(&ctx.xr);
        let world = World::new();

        let renderer = Renderer::new(UVec2::new(config.width, config.height), ctx);
        let gpu_resources = GpuResources::new(&ctx.device, &ctx.queue);

        let aspect_ratio = config.width as f32 / config.height as f32;

        Self {
            input_handler,
            world,
            camera_controller: CameraController::new(),
            renderer,
            render_settings: RenderSettings::default(),
            aspect_ratio,
            gpu_resources,
            frame_timer: Timer::new(),
            fps_counter: FpsCounter::new(),
            first_frame: true,
            emitter_entity: None,
        }
    }

    fn egui(
        &mut self,
        ui: &mut egui::Context,
        _xr_camera_state: &XrCameraState,
        _command_encoder: &mut wgpu::CommandEncoder,
        _ctx: &wgpu_util::Context,
        _pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) {
        egui::Window::new("Terrarium - Massive").show(ui, |ui| {
            self.render_settings.egui(ui);
        });
    }

    fn update(
        &mut self,
        xr_camera_state: &mut XrCameraState,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
        _pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) {
        let delta_time = self.frame_timer.elapsed();
        self.frame_timer.reset();

        self.camera_controller
            .update(&self.input_handler, delta_time, xr_camera_state);
        self.camera_controller.update_xr_camera_state(
            self.aspect_ratio,
            self.render_settings.enable_taa,
            xr_camera_state,
        );

        if self.first_frame {
            self.first_frame = false;

            let model = ugm::Model::read_from_buffer(
                &std::fs::read("examples/massive/assets/TestSceneBig.ugm")
                .expect("It looks like you're missing the TestScene.glb model. Please download it from here https://drive.google.com/file/d/1Phta9UH7fvtCCOQMh3c0YxrL6kYzjcJc/view?usp=drive_link and place it in the assets folder."),
            )
            .unwrap();
            self.world.spawn_model(
                &model,
                Transform::default(),
                true,
                None,
                &mut self.gpu_resources,
                command_encoder,
                ctx,
            );

            let model = ugm::Model::read_from_buffer(
                &std::fs::read("examples/massive/assets/Emitter.ugm")
                .expect("It looks like you're missing the TestScene.glb model. Please download it from here https://drive.google.com/file/d/1Phta9UH7fvtCCOQMh3c0YxrL6kYzjcJc/view?usp=drive_link and place it in the assets folder."),
            )
            .unwrap();

            for x in 0..10 {
                for y in 0..10 {
                    self.emitter_entity = Some(self.world.spawn_model(
                        &model,
                        Transform::new(
                            (UP * 4.0 + RIGHT * x as f32 * 2.0 + FORWARD * y as f32 * 2.0)
                                - Vec3::new(10.0, 0.0, -10.0),
                            Quat::IDENTITY,
                            Vec3::splat(5.0),
                        ),
                        false,
                        None,
                        &mut self.gpu_resources,
                        command_encoder,
                        ctx,
                    ));
                }
            }

            self.gpu_resources.mark_statics_dirty();
        }

        {
            self.world
                .entities_mut::<TransformComponent>()
                .get_mut(self.emitter_entity.unwrap())
                .unwrap()
                .rotate_local(Quat::from_axis_angle(
                    RIGHT,
                    (delta_time * 10.0).to_radians(),
                ));
            self.world
                .entities::<TransformComponent>()
                .get(self.emitter_entity.unwrap())
                .unwrap()
                .mark_dirty(&self.world.entities::<TransformComponent>());
        }

        self.world.update();
        self.input_handler.update();
        self.fps_counter.end_frame();

        println!("FPS {}", self.fps_counter.fps());
    }

    fn render(
        &mut self,
        xr_camera_state: &mut XrCameraState,
        xr_camera_buffer: &wgpu::Buffer,
        render_target: &wgpu::Texture,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) {
        self.gpu_resources.debug_lines_mut().submit_line(
            Vec3::ZERO,
            RIGHT * 10000.0,
            Vec3::new(1.0, 0.0, 0.0),
        );
        self.gpu_resources.debug_lines_mut().submit_line(
            Vec3::ZERO,
            UP * 10000.0,
            Vec3::new(0.0, 1.0, 0.0),
        );
        self.gpu_resources.debug_lines_mut().submit_line(
            Vec3::ZERO,
            FORWARD * 10000.0,
            Vec3::new(0.0, 0.0, 1.0),
        );

        self.renderer.render(
            &mut RenderParameters {
                render_settings: &self.render_settings,
                world: self.world.specs(),
                xr_camera_state,
                xr_camera_buffer,
                render_target,
                gpu_resources: &mut self.gpu_resources,
            },
            command_encoder,
            ctx,
            pipeline_database,
        );
    }

    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) {
        self.aspect_ratio = config.width as f32 / config.height as f32;

        self.renderer
            .resize(UVec2::new(config.width, config.height), ctx);
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
        Renderer::required_features()
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
            max_binding_array_elements_per_shader_stage: 1024,
            max_storage_textures_per_shader_stage: 8,
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
