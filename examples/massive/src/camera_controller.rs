use glam::{Mat4, Quat, Vec2, Vec3};
use terrarium::{
    helpers::input_handler::InputHandler,
    render_passes::taa_pass::TaaJitter,
    world::transform::{FORWARD, HORIZONTAL_MASK, RIGHT, UP},
    xr::{XrCameraState, XrHand},
};
use winit::keyboard::KeyCode;

pub struct CameraController {
    pub translation_speed: f32,
    pub look_sensitivity: f32,
    stage_translation: Vec3,
    stage_vertical_rotation: Quat,
    stage_horizontal_rotation: Quat,
    locked: bool,
    frame_idx: u32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            translation_speed: 1.0,
            look_sensitivity: 0.3,
            stage_translation: Vec3::new(2.0, 2.0, 0.0),
            stage_vertical_rotation: Quat::IDENTITY,
            stage_horizontal_rotation: Quat::IDENTITY,
            locked: true,
            frame_idx: 0,
        }
    }
}

impl CameraController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(
        &mut self,
        input: &InputHandler,
        delta_time: f32,
        xr_camera_state: &mut XrCameraState,
    ) {
        if input.current().keyboard_key(KeyCode::F1) && !input.prev().keyboard_key(KeyCode::F1) {
            self.locked = !self.locked;
        }

        if !self.locked {
            let (_, rotation, _) = xr_camera_state.stage_to_view_space[0]
                .inverse()
                .to_scale_rotation_translation();
            let rotation =
                rotation * (self.stage_vertical_rotation * self.stage_horizontal_rotation);
            let forward = rotation * FORWARD;
            let right = rotation * RIGHT;

            let mut velocity = Vec3::ZERO;
            if input.current().keyboard_key(KeyCode::KeyW) {
                velocity += forward * HORIZONTAL_MASK;
            }
            if input.current().keyboard_key(KeyCode::KeyS) {
                velocity -= forward * HORIZONTAL_MASK;
            }
            if input.current().keyboard_key(KeyCode::KeyD) {
                velocity += right * HORIZONTAL_MASK;
            }
            if input.current().keyboard_key(KeyCode::KeyA) {
                velocity -= right * HORIZONTAL_MASK;
            }
            if input.current().keyboard_key(KeyCode::KeyE) {
                velocity += UP;
            }
            if input.current().keyboard_key(KeyCode::KeyQ) {
                velocity -= UP;
            }

            if let Some(thumbstick) = input
                .current()
                .xr_hand(XrHand::Right)
                .analog_2d("/input/thumbstick")
            {
                velocity += (forward * thumbstick.y + right * thumbstick.x) * HORIZONTAL_MASK;
            }
            if let Some(y) = input
                .current()
                .xr_hand(XrHand::Left)
                .digital("/input/y/click")
            {
                if y {
                    velocity += UP;
                }
            }
            if let Some(x) = input
                .current()
                .xr_hand(XrHand::Left)
                .digital("/input/x/click")
            {
                if x {
                    velocity -= UP;
                }
            }

            if velocity.length() > 0.0 {
                let translation_speed = if input.current().keyboard_key(KeyCode::Space) {
                    self.translation_speed * 5.0
                } else if input.current().keyboard_key(KeyCode::ControlLeft) {
                    self.translation_speed * 0.2
                } else if let Some(trigger_value) = input
                    .current()
                    .xr_hand(XrHand::Right)
                    .analog("/input/trigger/value")
                {
                    self.translation_speed * (1.0 + trigger_value * 5.0)
                } else {
                    self.translation_speed
                };
                self.stage_translation += velocity.normalize() * delta_time * translation_speed;
            }

            if let Some(thumbstick) = input
                .current()
                .xr_hand(XrHand::Left)
                .analog_2d("/input/thumbstick")
            {
                self.stage_vertical_rotation *= Quat::from_axis_angle(
                    UP,
                    (-thumbstick.x * self.look_sensitivity * 4.0).to_radians(),
                );
            }

            self.stage_vertical_rotation *= Quat::from_axis_angle(
                UP,
                (-input.current().mouse_motion().x * self.look_sensitivity).to_radians(),
            );
            self.stage_horizontal_rotation *= Quat::from_axis_angle(
                RIGHT,
                (-input.current().mouse_motion().y * self.look_sensitivity).to_radians(),
            );
        }

        self.frame_idx += 1;
    }

    pub fn update_xr_camera_state(
        &self,
        aspect_ratio: f32,
        jitter: bool,
        xr_camera_state: &mut XrCameraState,
    ) {
        for i in 0..2 {
            xr_camera_state.view_to_clip_space[i] = Mat4::perspective_rh(
                60.0f32.to_radians(),
                aspect_ratio,
                xr_camera_state.z_near,
                xr_camera_state.z_far,
            );
        }

        xr_camera_state.stage_translation = self.stage_translation;
        xr_camera_state.stage_rotation =
            self.stage_vertical_rotation * self.stage_horizontal_rotation;

        xr_camera_state.jitter = if jitter {
            TaaJitter::frame_jitter(self.frame_idx)
        } else {
            Vec2::ZERO
        };
    }
}
