use futures::executor::block_on;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::{
    render_passes::blit_pass::{self, BlitPassParameters},
    wgpu_util::{self},
    xr::{XrCameraData, XrCameraState},
};

pub trait AppLoop: 'static + Sized {
    fn new(
        config: &wgpu::SurfaceConfiguration,
        ctx: &wgpu_util::Context,
        window: Arc<Window>,
    ) -> Self;

    fn update(
        &mut self,
        xr_camera_state: &mut XrCameraState,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    );
    fn render(
        &mut self,
        xr_camera_state: &mut XrCameraState,
        xr_camera_buffer: &wgpu::Buffer,
        render_target: &wgpu::Texture,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    );
    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context);

    #[cfg(feature = "egui")]
    fn egui(
        &mut self,
        _ui: &mut egui::Context,
        _xr_camera_state: &XrCameraState,
        _command_encoder: &mut wgpu::CommandEncoder,
        _ctx: &wgpu_util::Context,
        _pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) {
    }

    fn window_event(&mut self, _event: winit::event::WindowEvent) {}
    fn device_event(&mut self, _event: winit::event::DeviceEvent) {}
    fn xr_post_frame(&mut self, _xr_frame_state: &openxr::FrameState, _xr: &wgpu_util::XrContext) {}

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

            #[cfg(feature = "egui")]
            state.egui_renderer.handle_input(&state.window, &event);
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let mut command_encoder = state
                        .context
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                    #[cfg(feature = "egui")]
                    {
                        let mut ui = state.egui_renderer.begin_frame(&state.window);

                        state.app_loop.egui(
                            &mut ui,
                            &state.xr_camera_state,
                            &mut command_encoder,
                            &state.context,
                            &mut state.pipeline_database,
                        );
                        state.egui_renderer.end_frame(ui);
                    }

                    state.app_loop.update(
                        &mut state.xr_camera_state,
                        &mut command_encoder,
                        &state.context,
                        &mut state.pipeline_database,
                    );

                    let is_minimized =
                        state.surface.config().width == 1 || state.surface.config().height == 1;

                    let (xr_views, frame, xr_frame_state) = if !is_minimized {
                        state.app_loop.render(
                            &mut state.xr_camera_state,
                            &state.xr_camera_buffer,
                            &state.rt_texture,
                            &mut command_encoder,
                            &state.context,
                            &mut state.pipeline_database,
                        );

                        let rt_texture_view =
                            state.rt_texture.create_view(&wgpu::TextureViewDescriptor {
                                dimension: Some(wgpu::TextureViewDimension::D2Array),
                                array_layer_count: Some(2),
                                mip_level_count: Some(1),
                                ..Default::default()
                            });

                        #[cfg(feature = "egui")]
                        {
                            let screen_descriptor = crate::egui_renderer::ScreenDescriptor {
                                size_in_pixels: [
                                    state.window.inner_size().width,
                                    state.window.inner_size().height,
                                ],
                                pixels_per_point: state.window.scale_factor() as f32,
                            };

                            state.egui_renderer.draw(
                                &state.window,
                                &rt_texture_view,
                                screen_descriptor,
                                &state.context.device,
                                &state.context.queue,
                                &mut command_encoder,
                            );
                        }

                        let frame = state.surface.acquire(&state.context);
                        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                            format: Some(state.surface.config().view_formats[0]),
                            ..wgpu::TextureViewDescriptor::default()
                        });
                        blit_pass::encode(
                            &BlitPassParameters {
                                src_view: &rt_texture_view,
                                dst_view: &view,
                                multiview: None,
                                view_index_override: None,
                                target_format: state.surface.config().view_formats[0],
                            },
                            &state.context.device,
                            &mut command_encoder,
                            &mut state.pipeline_database,
                        );

                        let xr_frame_state = if let Some(xr) = &mut state.context.xr {
                            xr.pre_frame().unwrap()
                        } else {
                            None
                        };
                        if let Some(xr) = &mut state.context.xr {
                            xr.pre_render().unwrap();
                        }

                        let xr_views = if let Some(xr) = &mut state.context.xr {
                            if let Some(xr_frame_state) = xr_frame_state {
                                state.app_loop.xr_post_frame(&xr_frame_state, xr);

                                let xr_views = xr
                                    .post_frame(
                                        &rt_texture_view,
                                        xr_frame_state,
                                        &state.context.device,
                                        &mut command_encoder,
                                        &mut state.pipeline_database,
                                    )
                                    .unwrap();

                                Some(xr_views)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if let Some(xr_views) = &xr_views {
                            state
                                .xr_camera_state
                                .stage_to_view_space_from_openxr_views(xr_views);
                            state
                                .xr_camera_state
                                .view_to_clip_space_from_openxr_views(xr_views);
                        } else {
                            state.xr_camera_state.default_stage_to_view_space();
                        }

                        let xr_camera_data = [
                            state.xr_camera_state.calculate_camera_data(),
                            state.prev_xr_camera_data,
                        ];
                        state.context.queue.write_buffer(
                            &state.xr_camera_buffer,
                            0,
                            bytemuck::bytes_of(&xr_camera_data),
                        );
                        state.prev_xr_camera_data = xr_camera_data[0];

                        (xr_views, Some(frame), xr_frame_state)
                    } else {
                        (None, None, None)
                    };

                    state.context.queue.submit(Some(command_encoder.finish()));

                    if !is_minimized {
                        if let Some(xr) = &mut state.context.xr {
                            if let (Some(xr_frame_state), Some(xr_views)) =
                                (xr_frame_state, xr_views)
                            {
                                xr.post_frame_submit(xr_frame_state, &xr_views).unwrap();
                            }
                        }

                        frame.unwrap().present();
                    }

                    state.window.request_redraw();
                }

                self.frame_idx += 1;
            }
            WindowEvent::Resized(mut size) => {
                if let Some(state) = &mut self.state {
                    size.width = size.width.max(1);
                    size.height = size.height.max(1);

                    state.surface.resize(&state.context, size);
                    state.rt_texture = State::<R>::create_rt_texture(
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
    context: wgpu_util::Context,
    pipeline_database: wgpu_util::PipelineDatabase,
    xr_camera_state: XrCameraState,
    prev_xr_camera_data: XrCameraData,
    xr_camera_buffer: wgpu::Buffer,
    rt_texture: wgpu::Texture,
    app_loop: R,

    #[cfg(feature = "egui")]
    egui_renderer: crate::egui_renderer::EguiRenderer,
}

impl<R: AppLoop> State<R> {
    async fn from_window(
        mut surface: wgpu_util::Surface,
        window: Arc<Window>,
        no_gpu_validation: bool,
    ) -> Self {
        let context = if let Ok(context) = wgpu_util::Context::init_with_xr(
            R::required_features(),
            R::required_limits(),
            no_gpu_validation,
        ) {
            context
        } else {
            wgpu_util::Context::init_with_window(
                &mut surface,
                window.clone(),
                R::optional_features(),
                R::required_features(),
                R::required_downlevel_capabilities(),
                R::required_limits(),
                no_gpu_validation,
            )
            .await
        };

        surface.resume(&context, window.clone(), true);

        if let Some(xr) = &context.xr {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                xr.view_configs[0].recommended_image_rect_width,
                xr.view_configs[0].recommended_image_rect_height,
            ));
        }

        let app_loop = R::new(surface.config(), &context, window.clone());

        let rt_texture = Self::create_rt_texture(surface.config(), &context.device);

        let xr_camera_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::xr_camera"),
            size: size_of::<XrCameraData>() as u64 * 2,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let xr_connected = context.xr.is_some();
        let xr_camera_state = XrCameraState::new(0.01, 10000.0, xr_connected);

        let mut pipeline_database = wgpu_util::PipelineDatabase::new();

        #[cfg(feature = "egui")]
        let egui_renderer = crate::egui_renderer::EguiRenderer::new(
            wgpu::TextureFormat::Rgba16Float,
            1,
            &window,
            &context.device,
            &mut pipeline_database,
        );

        Self {
            window,
            surface,
            context,
            pipeline_database,
            xr_camera_state,
            prev_xr_camera_data: XrCameraData::default(),
            xr_camera_buffer,
            rt_texture,
            app_loop,

            #[cfg(feature = "egui")]
            egui_renderer,
        }
    }

    fn create_rt_texture(
        surface_config: &wgpu::SurfaceConfiguration,
        device: &wgpu::Device,
    ) -> wgpu::Texture {
        let width = surface_config.width;
        let height = surface_config.height;
        let mip_level_count = (((width.max(height) as f32).log2()).floor() + 1.0) as u32;

        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrarium::render_target"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 2,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        })
    }
}
