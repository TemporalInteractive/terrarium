use std::collections::HashMap;

use glam::Vec2;
use openxr::ActionInput;
use winit::{
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    wgpu_util,
    xr::{XrHand, XrInputActions, XrPose},
};

#[derive(Debug, Clone)]
pub struct XrHandState {
    hand: XrHand,
    digital: HashMap<String, bool>,
    analog: HashMap<String, f32>,
    analog_2d: HashMap<String, Vec2>,
    pose: HashMap<String, XrPose>,
}

impl XrHandState {
    fn new(hand: XrHand) -> Self {
        Self {
            hand,
            digital: HashMap::new(),
            analog: HashMap::new(),
            analog_2d: HashMap::new(),
            pose: HashMap::new(),
        }
    }

    pub fn digital(&self, id: &str) -> Option<bool> {
        self.digital
            .get(&format!("/user/hand/{}{}", self.hand, id))
            .copied()
    }

    pub fn analog(&self, id: &str) -> Option<f32> {
        self.analog
            .get(&format!("/user/hand/{}{}", self.hand, id))
            .copied()
    }

    pub fn analog_2d(&self, id: &str) -> Option<Vec2> {
        self.analog_2d
            .get(&format!("/user/hand/{}{}", self.hand, id))
            .copied()
    }

    pub fn pose(&self, id: &str) -> Option<XrPose> {
        self.pose
            .get(&format!("/user/hand/{}{}", self.hand, id))
            .copied()
    }
}

#[derive(Debug, Clone)]
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
            xr_hand: [
                XrHandState::new(XrHand::Left),
                XrHandState::new(XrHand::Right),
            ],
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

    pub fn xr_hand(&self, hand: XrHand) -> &XrHandState {
        &self.xr_hand[hand as usize]
    }
}

pub struct InputHandler {
    state: InputState,
    prev_state: InputState,
    xr_input_actions: Option<XrInputActions>,
}

impl InputHandler {
    pub fn new(xr: &Option<wgpu_util::XrContext>) -> Self {
        let xr_input_actions = xr.as_ref().map(|xr| XrInputActions::new(xr).unwrap());

        Self {
            state: InputState::default(),
            prev_state: InputState::default(),
            xr_input_actions,
        }
    }

    pub fn current(&self) -> &InputState {
        &self.state
    }

    fn current_mut(&mut self) -> &mut InputState {
        &mut self.state
    }

    pub fn prev(&self) -> &InputState {
        &self.prev_state
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

    pub fn handle_device_input(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.current_mut().mouse_motion += Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    pub fn handle_xr_input(
        &mut self,
        xr_frame_state: &openxr::FrameState,
        xr: &wgpu_util::XrContext,
    ) {
        let mut xr_hand = [
            XrHandState::new(XrHand::Left),
            XrHandState::new(XrHand::Right),
        ];

        if let Some(xr_input_actions) = &self.xr_input_actions {
            xr.session
                .sync_actions(&[(&xr_input_actions.action_set).into()])
                .unwrap();

            for (i, xr_hand) in xr_hand.iter_mut().enumerate() {
                for (path, action) in &xr_input_actions.hand_input_actions[i].digital {
                    let value =
                        if let Ok(value) = bool::get(action, &xr.session, openxr::Path::NULL) {
                            value.current_state
                        } else {
                            false
                        };

                    xr_hand.digital.insert(path.to_owned(), value);
                }

                for (path, action) in &xr_input_actions.hand_input_actions[i].analog {
                    let value = if let Ok(value) = f32::get(action, &xr.session, openxr::Path::NULL)
                    {
                        value.current_state
                    } else {
                        0.0
                    };

                    xr_hand.analog.insert(path.to_owned(), value);
                }

                for (path, action) in &xr_input_actions.hand_input_actions[i].analog_2d {
                    let value = if let Ok(value) =
                        openxr::Vector2f::get(action, &xr.session, openxr::Path::NULL)
                    {
                        Vec2::new(value.current_state.x, value.current_state.y)
                    } else {
                        Vec2::ZERO
                    };

                    xr_hand.analog_2d.insert(path.to_owned(), value);
                }

                for (path, (action, space)) in &xr_input_actions.hand_input_actions[i].pose {
                    if action.is_active(&xr.session, openxr::Path::NULL).unwrap() {
                        let value = XrPose::from_openxr(
                            &space
                                .locate(&xr.stage, xr_frame_state.predicted_display_time)
                                .unwrap()
                                .pose,
                        );

                        xr_hand.pose.insert(path.to_owned(), value);
                    }
                }
            }
        }

        self.current_mut().xr_hand = xr_hand;
    }

    pub fn update(&mut self) {
        self.prev_state = self.state.clone();
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
