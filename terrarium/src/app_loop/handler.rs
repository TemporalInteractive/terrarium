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

use super::AppLoop;
use crate::wgpu_util;

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
                    // xr preframe

                    let frame = state.surface.acquire(&state.context);
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                        format: Some(state.surface.config().view_formats[0]),
                        ..wgpu::TextureViewDescriptor::default()
                    });
                    if state.app_loop.render(&view, &state.context) {
                        event_loop.exit();
                    }
                    // xr post frame -> camera & joystick positions

                    // update camera data buffer

                    // update input manager with joystick data (only usable for next frame), meaning 1 frame delay

                    // submit cmd encoder to queue

                    // xr post frame submit

                    frame.present();

                    state.window.request_redraw();
                }

                self.frame_idx += 1;
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.surface.resize(&state.context, size);

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
    app_loop: R,
}

impl<R: AppLoop> State<R> {
    async fn from_window(
        mut surface: wgpu_util::Surface,
        window: Arc<Window>,
        no_gpu_validation: bool,
    ) -> Self {
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

        surface.resume(&context, window.clone(), true);

        let app_loop = R::new(surface.config(), &context, window.clone());

        Self {
            window,
            surface,
            context,
            app_loop,
        }
    }
}
