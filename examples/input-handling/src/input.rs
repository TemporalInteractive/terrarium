use std::collections::HashMap;

use glam::{Quat, Vec2, Vec3};
use winit::{
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub enum XrHand {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct XrPose {
    pub orientation: Quat,
    pub position: Vec3,
}

#[derive(Debug, Default, Clone)]
struct XrHandState {
    digital: HashMap<String, bool>,
    analog: HashMap<String, f32>,
    analog_2d: HashMap<String, Vec2>,
    pose: HashMap<String, XrPose>,
}

#[derive(Debug)]
pub struct InputState {
    keyboard_keys: [bool; 512],
    mouse_buttons: [bool; 32],
    mouse_position: Vec2,
    mouse_motion: Vec2,
    mouse_wheel: f32,

    xr_hand: [XrHandState; 2],
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keyboard_keys: [false; 512],
            mouse_buttons: [false; 32],
            mouse_position: Vec2::ZERO,
            mouse_motion: Vec2::ZERO,
            mouse_wheel: 0.0,
            xr_hand: std::array::from_fn(|_| Default::default()),
        }
    }
}

impl InputState {
    pub fn keyboard_key(&self, key_code: KeyCode) -> bool {
        self.keyboard_keys[key_code as usize]
    }

    pub fn mouse_button(&self, button: MouseButton) -> bool {
        self.mouse_buttons[mouse_button_to_usize(&button)]
    }

    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_position
    }

    pub fn mouse_motion(&self) -> Vec2 {
        self.mouse_motion
    }

    pub fn mouse_wheel(&self) -> f32 {
        self.mouse_wheel
    }
}

#[derive(Debug, Default)]
pub struct InputHandler {
    state: [InputState; 2],
    frame_idx: u32,
}

impl InputHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> &InputState {
        &self.state[(self.frame_idx as usize) % 2]
    }

    fn current_mut(&mut self) -> &mut InputState {
        &mut self.state[(self.frame_idx as usize) % 2]
    }

    pub fn prev(&self) -> &InputState {
        &self.state[(self.frame_idx as usize + 1) % 2]
    }

    pub fn handle_window_input(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, _) => *x,
                    winit::event::MouseScrollDelta::PixelDelta(x) => x.x as f32,
                };
                self.current_mut().mouse_wheel += delta;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.current_mut().mouse_position = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.repeat {
                    if let PhysicalKey::Code(key_code) = event.physical_key {
                        self.current_mut().keyboard_keys[key_code as usize] =
                            event.state == ElementState::Pressed;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.current_mut().mouse_buttons[mouse_button_to_usize(button)] =
                    *state == ElementState::Pressed;
            }
            _ => {}
        }
    }

    pub fn handle_device_input(&mut self, device_event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = device_event {
            self.current_mut().mouse_motion += Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    pub fn update(&mut self) {
        self.frame_idx += 1;
        self.current_mut().mouse_motion = Vec2::ZERO;
    }
}

fn mouse_button_to_usize(button: &MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(i) => 4 + *i as usize,
    }
}
