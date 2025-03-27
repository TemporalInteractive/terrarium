use futures::executor::block_on;
use glam::Mat4;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use super::AppLoop;
use crate::{
    render_passes::blit_pass::{self, BlitPassParameters},
    wgpu_util::{self, PipelineDatabase},
    xr::XrState,
};

#[derive(Debug, Clone)]
pub struct AppLoopHandlerCreateDesc {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizeable: bool,
    pub maximized: bool,
    pub no_gpu_validation: bool,
}

impl Default for AppLoopHandlerCreateDesc {
    fn default() -> Self {
        Self {
            title: "Terrarium".to_owned(),
            width: 1920,
            height: 1080,
            resizeable: false,
            maximized: false,
            no_gpu_validation: false,
        }
    }
}

pub struct AppLoopHandler<R: AppLoop> {
    create_desc: AppLoopHandlerCreateDesc,
    state: Option<State<R>>,
    frame_idx: u32,
}

impl<R: AppLoop> AppLoopHandler<R> {
    pub fn new(create_desc: &AppLoopHandlerCreateDesc) -> Self {
        Self {
            create_desc: create_desc.to_owned(),
            state: None,
            frame_idx: 0,
        }
    }

    pub fn run(mut self) -> Result<(), EventLoopError> {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self)
    }
}

impl<R: AppLoop> ApplicationHandler for AppLoopHandler<R> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let surface = if let Some(state) = self.state.take() {
            state.surface
        } else {
            wgpu_util::Surface::new()
        };

        let window_attributes = Window::default_attributes()
            .with_title(&self.create_desc.title)
            .with_resizable(self.create_desc.resizeable)
            .with_inner_size(PhysicalSize::new(
                self.create_desc.width,
                self.create_desc.height,
            ))
            .with_maximized(self.create_desc.maximized);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.state = Some(block_on(State::<R>::from_window(
            surface,
            window,
            self.create_desc.no_gpu_validation,
        )));
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            state.surface.suspend();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(state) = &mut self.state {
            state.app_loop.window_event(event.clone());
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let xr_frame_state = if let Some(xr_state) = &mut state.xr_state {
                        xr_state.pre_frame().unwrap()
                    } else {
                        None
                    };

                    let mut command_encoder = state.app_loop.render(
                        &state.xr_camera_buffer,
                        &state.rt_texture_view,
                        &state.context,
                        &mut state.pipeline_database,
                    );

                    let frame = state.surface.acquire(&state.context);
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                        format: Some(state.surface.config().view_formats[0]),
                        ..wgpu::TextureViewDescriptor::default()
                    });
                    blit_pass::encode(
                        &BlitPassParameters {
                            src_view: &state.rt_texture_view,
                            dst_view: &view,
                            multiview: None,
                            target_format: state.surface.config().view_formats[0],
                        },
                        &state.context.device,
                        &mut command_encoder,
                        &mut state.pipeline_database,
                    );

                    let xr_post_frame_data = if let Some(xr_state) = &mut state.xr_state {
                        if let Some(xr_frame_state) = xr_frame_state {
                            let xr_post_frame_data = xr_state
                                .post_frame(
                                    &state.rt_texture_view,
                                    &state.xr_camera_buffer,
                                    xr_frame_state,
                                    &state.context.device,
                                    &state.context.queue,
                                    &mut command_encoder,
                                    &mut state.pipeline_database,
                                )
                                .unwrap();

                            // xr post frame -> camera & joystick positions

                            // update camera data buffer

                            // update input manager with joystick data (only usable for next frame), meaning 1 frame delay

                            Some(xr_post_frame_data)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    state.context.queue.submit(Some(command_encoder.finish()));

                    if let Some(xr_state) = &mut state.xr_state {
                        if let (Some(xr_frame_state), Some(xr_post_frame_data)) =
                            (xr_frame_state, xr_post_frame_data)
                        {
                            xr_state
                                .post_frame_submit(xr_frame_state, &xr_post_frame_data.views)
                                .unwrap();
                        }
                    }

                    frame.present();

                    state.window.request_redraw();
                }

                self.frame_idx += 1;
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.surface.resize(&state.context, size);
                    state.rt_texture_view = State::<R>::create_rt_texture_view(
                        state.surface.config(),
                        &state.context.device,
                    );

                    state
                        .app_loop
                        .resize(state.surface.config(), &state.context);

                    state.window.request_redraw();
                }
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(state) = &mut self.state {
            state.app_loop.device_event(event.clone());
        }
    }
}

struct State<R: AppLoop> {
    window: Arc<Window>,
    surface: wgpu_util::Surface,
    context: Arc<wgpu_util::Context>,
    pipeline_database: wgpu_util::PipelineDatabase,
    xr_state: Option<XrState>,
    xr_camera_buffer: wgpu::Buffer,
    rt_texture_view: wgpu::TextureView,
    app_loop: R,
}

impl<R: AppLoop> State<R> {
    async fn from_window(
        mut surface: wgpu_util::Surface,
        window: Arc<Window>,
        no_gpu_validation: bool,
    ) -> Self {
        let (context, xr_state) = if let Ok((context, xr_state)) =
            XrState::initialize_with_wgpu(R::required_features(), R::required_limits())
        {
            (Arc::new(context), Some(xr_state))
        } else {
            let context = Arc::new(
                wgpu_util::Context::init_with_window(
                    &mut surface,
                    window.clone(),
                    R::optional_features(),
                    R::required_features(),
                    R::required_downlevel_capabilities(),
                    R::required_limits(),
                    no_gpu_validation,
                )
                .await,
            );

            (context, None)
        };

        surface.resume(&context, window.clone(), true);

        let app_loop = R::new(surface.config(), &context, window.clone());

        let rt_texture_view = Self::create_rt_texture_view(surface.config(), &context.device);

        let xr_camera_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::xr_camera"),
            size: size_of::<Mat4>() as u64 * 2,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        Self {
            window,
            surface,
            context,
            pipeline_database: wgpu_util::PipelineDatabase::new(),
            xr_state,
            xr_camera_buffer,
            rt_texture_view,
            app_loop,
        }
    }

    fn create_rt_texture_view(
        surface_config: &wgpu::SurfaceConfiguration,
        device: &wgpu::Device,
    ) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrarium::render_target"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            ..Default::default()
        })
    }
}
