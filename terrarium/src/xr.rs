use std::{collections::HashMap, fmt::Display};

use anyhow::Result;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4, Vec4Swizzles};

use crate::{
    wgpu_util,
    world::transform::{FORWARD, UP},
};

pub const WGPU_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const VK_COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const VIEW_TYPE: openxr::ViewConfigurationType = openxr::ViewConfigurationType::PRIMARY_STEREO;

#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct XrCameraData {
    pub view_to_clip_space: [Mat4; 2],
    pub world_to_view_space: [Mat4; 2],
    pub clip_to_view_space: [Mat4; 2],
    pub view_to_world_space: [Mat4; 2],
    pub jitter: Vec2,
    _padding0: u32,
    _padding1: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct XrCameraState {
    pub stage_to_view_space: [Mat4; 2],
    pub view_to_clip_space: [Mat4; 2],
    pub stage_translation: Vec3,
    pub stage_rotation: Quat,
    pub camera_rotation_offset: Quat,
    pub z_near: f32,
    pub z_far: f32,
    pub jitter: Vec2,
}

impl XrCameraState {
    pub fn new(z_near: f32, z_far: f32) -> Self {
        Self {
            stage_to_view_space: [Mat4::IDENTITY; 2],
            view_to_clip_space: [Mat4::IDENTITY; 2],
            stage_translation: Vec3::ZERO,
            stage_rotation: Quat::IDENTITY,
            camera_rotation_offset: Quat::IDENTITY,
            z_near,
            z_far,
            jitter: Vec2::ZERO,
        }
    }

    pub fn stage_to_view_space_from_openxr_views(&mut self, views: &[openxr::View]) {
        for (i, view) in views.iter().enumerate() {
            let pose = XrPose::from_openxr(&view.pose);

            self.stage_to_view_space[i] = Mat4::look_at_rh(
                pose.translation,
                pose.translation + pose.orientation * FORWARD,
                pose.orientation * UP,
            );
        }
    }

    pub fn view_to_clip_space_from_openxr_views(&mut self, views: &[openxr::View]) {
        for (i, view) in views.iter().enumerate() {
            let tan_left = view.fov.angle_left.tan();
            let tan_right = view.fov.angle_right.tan();
            let tan_down = view.fov.angle_down.tan();
            let tan_up = view.fov.angle_up.tan();

            let tan_width = tan_right - tan_left;
            let tan_height = tan_up - tan_down;

            let a11 = 2.0 / tan_width;
            let a22 = 2.0 / tan_height;
            let a31 = (tan_right + tan_left) / tan_width;
            let a32 = (tan_up + tan_down) / tan_height;
            let a33 = -self.z_far / (self.z_far - self.z_near);
            let a43 = -(self.z_far * self.z_near) / (self.z_far - self.z_near);

            self.view_to_clip_space[i] = glam::Mat4::from_cols_array(&[
                a11, 0.0, 0.0, 0.0, 0.0, a22, 0.0, 0.0, a31, a32, a33, -1.0, 0.0, 0.0, a43, 0.0,
            ]);
        }
    }

    pub fn calculate_camera_data(&self) -> XrCameraData {
        let world_to_view_space: [Mat4; 2] = std::array::from_fn(|i| {
            let (_, view_rotation, view_translation) = self.stage_to_view_space[i]
                .inverse()
                .to_scale_rotation_translation();

            // let head_position = Vec3::new(0.0, view_translation.y, 0.0);

            // let rotated_view_translation =
            //     self.camera_rotation_offset * (view_translation - head_position) + head_position;
            // let rotated_view_rotation = self.camera_rotation_offset * view_rotation;

            //let head_position = Vec3::new(0.0, view_translation.y, 0.0);

            let rotated_view_translation = self.stage_rotation * view_translation;
            //self.camera_rotation_offset * (view_translation - head_position) + head_position;
            let rotated_view_rotation = self.stage_rotation * view_rotation;

            let mut center = rotated_view_translation + self.stage_translation;
            let forward = rotated_view_rotation * FORWARD;
            let up = rotated_view_rotation * UP;

            Mat4::look_at_rh(center, center + forward, up)
        });

        XrCameraData {
            view_to_clip_space: self.view_to_clip_space,
            world_to_view_space,
            clip_to_view_space: [
                self.view_to_clip_space[0].inverse(),
                self.view_to_clip_space[1].inverse(),
            ],
            view_to_world_space: [
                world_to_view_space[0].inverse(),
                world_to_view_space[1].inverse(),
            ],
            jitter: self.jitter,
            _padding0: 0,
            _padding1: 1,
        }
    }
}

impl XrCameraData {
    pub fn generate_ray(&self, uv: Vec2, view_index: usize) -> (Vec3, Vec3) {
        let origin = (self.view_to_world_space[view_index] * Vec4::new(0.0, 0.0, 0.0, 1.0)).xyz();
        let target = self.clip_to_view_space[view_index] * Vec4::from((uv, 1.0, 1.0));
        let direction = (self.view_to_world_space[view_index]
            * Vec4::from((target.xyz().normalize(), 0.0)))
        .xyz();

        (origin, direction)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct XrPose {
    pub orientation: Quat,
    pub translation: Vec3,
}

impl XrPose {
    pub fn from_openxr(pose: &openxr::Posef) -> Self {
        let orientation = glam::quat(
            pose.orientation.x,
            pose.orientation.y,
            pose.orientation.z,
            pose.orientation.w,
        );
        let translation = glam::vec3(pose.position.x, pose.position.y, pose.position.z);

        Self {
            orientation,
            translation,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum XrHand {
    Left,
    Right,
}

impl Display for XrHand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left => f.write_str("left"),
            Self::Right => f.write_str("right"),
        }
    }
}

pub enum XrControllerProfile {
    Oculus,
    Vive,
    Valve,
    KhronosSimple,
}

impl XrControllerProfile {
    pub fn from_system_name(system_name: &str) -> Self {
        if system_name.contains("Oculus") {
            Self::Oculus
        } else if system_name.contains("Vive") {
            Self::Vive
        } else if system_name.contains("Valve") {
            Self::Valve
        } else {
            Self::KhronosSimple
        }
    }

    pub fn interaction_profile_path(&self) -> &'static str {
        match self {
            Self::Oculus => "/interaction_profiles/oculus/touch_controller",
            Self::Vive => "/interaction_profiles/htc/vive_controller",
            Self::Valve => "/interaction_profiles/valve/index_controller",
            Self::KhronosSimple => "/interaction_profiles/khr/simple_controller",
        }
    }
}

pub struct XrHandInputActions {
    pub digital: HashMap<String, openxr::Action<bool>>,
    pub analog: HashMap<String, openxr::Action<f32>>,
    pub analog_2d: HashMap<String, openxr::Action<openxr::Vector2f>>,
    pub pose: HashMap<String, (openxr::Action<openxr::Posef>, openxr::Space)>,
}

impl XrHandInputActions {
    pub fn new(
        hand: XrHand,
        profile: &XrControllerProfile,
        action_set: &openxr::ActionSet,
        xr: &wgpu_util::XrContext,
    ) -> Result<Self> {
        let mut digital = HashMap::new();
        let mut analog = HashMap::new();
        let mut analog_2d = HashMap::new();
        let mut pose = HashMap::new();

        let mut submit_digital_action = |input_path: &str| -> Result<()> {
            digital.insert(
                format!("/user/hand/{}/input/{}", hand, input_path),
                action_set
                    .create_action::<bool>(
                        &format!("{}_{}", hand, input_path.replace("/", "_")),
                        &format!("{}_{}", hand, input_path.replace("/", "_")),
                        &[],
                    )
                    .unwrap(),
            );

            Ok(())
        };

        let mut submit_analog_action = |input_path: &str| -> Result<()> {
            analog.insert(
                format!("/user/hand/{}/input/{}", hand, input_path),
                action_set
                    .create_action::<f32>(
                        &format!("{}_{}", hand, input_path.replace("/", "_")),
                        &format!("{}_{}", hand, input_path.replace("/", "_")),
                        &[],
                    )
                    .unwrap(),
            );

            Ok(())
        };

        let mut submit_analog_2d_action = |input_path: &str| -> Result<()> {
            analog_2d.insert(
                format!("/user/hand/{}/input/{}", hand, input_path),
                action_set
                    .create_action::<openxr::Vector2f>(
                        &format!("{}_{}", hand, input_path.replace("/", "_")),
                        &format!("{}_{}", hand, input_path.replace("/", "_")),
                        &[],
                    )
                    .unwrap(),
            );

            Ok(())
        };

        let mut submit_pose_action = |input_path: &str| -> Result<()> {
            let action = action_set
                .create_action::<openxr::Posef>(
                    &format!("{}_{}", hand, input_path.replace("/", "_")),
                    &format!("{}_{}", hand, input_path.replace("/", "_")),
                    &[],
                )
                .unwrap();

            let space = action
                .create_space(
                    xr.session.clone(),
                    openxr::Path::NULL,
                    openxr::Posef::IDENTITY,
                )
                .unwrap();

            pose.insert(
                format!("/user/hand/{}/input/{}", hand, input_path),
                (action, space),
            );

            Ok(())
        };

        match profile {
            XrControllerProfile::Oculus => {
                submit_analog_2d_action("thumbstick").unwrap();
                submit_digital_action("thumbstick/click").unwrap();
                submit_digital_action("thumbstick/touch").unwrap();
                submit_analog_action("squeeze/value").unwrap();
                submit_analog_action("trigger/value").unwrap();
                submit_pose_action("grip/pose").unwrap();
                submit_pose_action("aim/pose").unwrap();

                match hand {
                    XrHand::Left => {
                        submit_digital_action("menu/click").unwrap();
                        submit_digital_action("x/click").unwrap();
                        submit_digital_action("x/touch").unwrap();
                        submit_digital_action("y/click").unwrap();
                        submit_digital_action("y/touch").unwrap();
                    }
                    XrHand::Right => {
                        submit_digital_action("system/click").unwrap();
                        submit_digital_action("a/click").unwrap();
                        submit_digital_action("a/touch").unwrap();
                        submit_digital_action("b/click").unwrap();
                        submit_digital_action("b/touch").unwrap();
                    }
                }
            }
            XrControllerProfile::Vive => {
                submit_digital_action("system/click").unwrap();
                submit_digital_action("squeeze/click").unwrap();
                submit_digital_action("menu/click").unwrap();
                submit_analog_action("trigger/value").unwrap();
                submit_digital_action("trigger/click").unwrap();
                submit_analog_2d_action("trackpad").unwrap();
                submit_digital_action("trackpad/click").unwrap();
                submit_digital_action("trackpad/touch").unwrap();
                submit_pose_action("grip/pose").unwrap();
                submit_pose_action("aim/pose").unwrap();
            }
            XrControllerProfile::Valve => {
                submit_digital_action("system/click").unwrap();
                submit_digital_action("system/touch").unwrap();
                submit_digital_action("a/click").unwrap();
                submit_digital_action("a/touch").unwrap();
                submit_digital_action("b/click").unwrap();
                submit_digital_action("b/touch").unwrap();
                submit_analog_action("squeeze/value").unwrap();
                submit_analog_action("squeeze/force").unwrap();
                submit_digital_action("trigger/click").unwrap();
                submit_analog_action("trigger/value").unwrap();
                submit_digital_action("trigger/touch").unwrap();
                submit_analog_2d_action("thumbstick").unwrap();
                submit_digital_action("thumbstick/click").unwrap();
                submit_digital_action("thumbstick/touch").unwrap();
                submit_analog_2d_action("trackpad").unwrap();
                submit_digital_action("trackpad/touch").unwrap();
                submit_analog_action("trackpad/force").unwrap();
                submit_pose_action("grip/pose").unwrap();
                submit_pose_action("aim/pose").unwrap();
            }
            XrControllerProfile::KhronosSimple => {
                submit_digital_action("menu/click").unwrap();
                submit_digital_action("system/click").unwrap();
                submit_pose_action("grip/pose").unwrap();
                submit_pose_action("aim/pose").unwrap();
            }
        }

        Ok(Self {
            digital,
            analog,
            analog_2d,
            pose,
        })
    }
}

pub struct XrInputActions {
    pub profile: XrControllerProfile,
    pub action_set: openxr::ActionSet,
    pub hand_input_actions: [XrHandInputActions; 2],
}

impl XrInputActions {
    pub fn new(xr: &wgpu_util::XrContext) -> Result<Self> {
        let system = xr
            .instance
            .system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .unwrap();
        let system_properties = xr.instance.system_properties(system).unwrap();

        let profile = XrControllerProfile::from_system_name(&system_properties.system_name);
        let interaction_profile_path = profile.interaction_profile_path();

        let action_set = xr
            .instance
            .create_action_set("input", "input pose information", 0)
            .unwrap();
        let hand_input_actions = [
            XrHandInputActions::new(XrHand::Left, &profile, &action_set, xr).unwrap(),
            XrHandInputActions::new(XrHand::Right, &profile, &action_set, xr).unwrap(),
        ];

        let mut bindings = vec![];
        for hand_input_actions in &hand_input_actions {
            for (path, action) in &hand_input_actions.digital {
                bindings.push(openxr::Binding::new(
                    action,
                    xr.instance.string_to_path(path).unwrap(),
                ));
            }
            for (path, action) in &hand_input_actions.analog {
                bindings.push(openxr::Binding::new(
                    action,
                    xr.instance.string_to_path(path).unwrap(),
                ));
            }
            for (path, action) in &hand_input_actions.analog_2d {
                bindings.push(openxr::Binding::new(
                    action,
                    xr.instance.string_to_path(path).unwrap(),
                ));
            }
            for (path, action) in &hand_input_actions.pose {
                bindings.push(openxr::Binding::new(
                    &action.0,
                    xr.instance.string_to_path(path).unwrap(),
                ));
            }
        }

        xr.instance
            .suggest_interaction_profile_bindings(
                xr.instance
                    .string_to_path(interaction_profile_path)
                    .unwrap(),
                &bindings,
            )
            .unwrap();
        xr.session.attach_action_sets(&[&action_set]).unwrap();

        Ok(Self {
            profile,
            action_set,
            hand_input_actions,
        })
    }
}
