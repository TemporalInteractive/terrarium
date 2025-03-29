use std::{collections::HashMap, fmt::Display};

use anyhow::Result;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

use crate::{wgpu_util, UP};

pub const WGPU_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const VK_COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const VIEW_TYPE: openxr::ViewConfigurationType = openxr::ViewConfigurationType::PRIMARY_STEREO;

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct XrCameraData {
    pub stage_to_clip_space: [Mat4; 2],
    pub world_to_stage_space: Mat4,
}

impl Default for XrCameraData {
    fn default() -> Self {
        Self {
            stage_to_clip_space: [Mat4::IDENTITY; 2],
            world_to_stage_space: Mat4::IDENTITY,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct XrPose {
    pub orientation: Quat,
    pub translation: Vec3,
}

impl XrPose {
    pub fn from_openxr(pose: &openxr::Posef) -> Self {
        // with enough sign errors anything is possible
        let orientation = {
            let o = pose.orientation;
            Quat::from_rotation_x(180.0f32.to_radians()) * glam::quat(o.w, o.z, o.y, o.x)
        };
        let translation = glam::vec3(-pose.position.x, pose.position.y, -pose.position.z);

        Self {
            orientation,
            translation,
        }
    }
}

pub fn openxr_view_to_view_proj(v: &openxr::View, z_near: f32, z_far: f32) -> Mat4 {
    let pose = XrPose::from_openxr(&v.pose);

    let view = Mat4::look_at_rh(
        pose.translation,
        pose.translation + pose.orientation * Vec3::Z, // FORWARD?
        pose.orientation * UP,
    );

    let [tan_left, tan_right, tan_down, tan_up] = [
        v.fov.angle_left,
        v.fov.angle_right,
        v.fov.angle_down,
        v.fov.angle_up,
    ]
    .map(f32::tan);
    let tan_width = tan_right - tan_left;
    let tan_height = tan_up - tan_down;

    let a11 = 2.0 / tan_width;
    let a22 = 2.0 / tan_height;

    let a31 = (tan_right + tan_left) / tan_width;
    let a32 = (tan_up + tan_down) / tan_height;
    let a33 = -z_far / (z_far - z_near);

    let a43 = -(z_far * z_near) / (z_far - z_near);

    let proj = glam::Mat4::from_cols_array(&[
        a11, 0.0, 0.0, 0.0, //
        0.0, a22, 0.0, 0.0, //
        a31, a32, a33, -1.0, //
        0.0, 0.0, a43, 0.0, //
    ]);

    proj * view
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

// pub struct XrState {
//     xr_instance: xr::Instance,
//     environment_blend_mode: xr::EnvironmentBlendMode,
//     session: xr::Session<xr::Vulkan>,
//     session_running: bool,
//     frame_wait: xr::FrameWaiter,
//     frame_stream: xr::FrameStream<xr::Vulkan>,

//     action_set: xr::ActionSet,
//     right_action: xr::Action<xr::Posef>,
//     left_action: xr::Action<xr::Posef>,
//     right_thumbstick_action: xr::Action<xr::Vector2f>,
//     right_space: xr::Space,
//     left_space: xr::Space,
//     stage: xr::Space,

//     event_storage: xr::EventDataBuffer,
//     views: Vec<openxr::ViewConfigurationView>,
//     swapchain: Option<Swapchain>,
// }

// impl XrState {
//     pub fn initialize_with_wgpu(
//         wgpu_features: wgpu::Features,
//         wgpu_limits: wgpu::Limits,
//     ) -> anyhow::Result<(wgpu_util::Context, XrState)> {
//         let action_set = xr_instance.create_action_set("input", "input pose information", 0).unwrap();
//         let right_action =
//             action_set.create_action::<xr::Posef>("right_hand", "Right Hand Controller", &[]).unwrap();
//         let left_action =
//             action_set.create_action::<xr::Posef>("left_hand", "Left Hand Controller", &[]).unwrap();
//         let right_thumbstick_action = action_set.create_action::<xr::Vector2f>(
//             "right_thumbstick",
//             "Right Hand Controller Thumbstick",
//             &[],
//         ).unwrap();
//         xr_instance.suggest_interaction_profile_bindings(
//             xr_instance.string_to_path("/interaction_profiles/oculus/touch_controller").unwrap(), // /interaction_profiles/khr/simple_controller
//             &[
//                 xr::Binding::new(
//                     &right_action,
//                     xr_instance.string_to_path("/user/hand/right/input/grip/pose").unwrap(),
//                 ),
//                 xr::Binding::new(
//                     &left_action,
//                     xr_instance.string_to_path("/user/hand/left/input/grip/pose").unwrap(),
//                 ),
//                 xr::Binding::new(
//                     &right_thumbstick_action,
//                     xr_instance.string_to_path("/user/hand/left/input/thumbstick").unwrap(),
//                 ),
//             ],
//         ).unwrap();
//         session.attach_action_sets(&[&action_set]).unwrap();
//         let right_space =
//             right_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY).unwrap();
//         let left_space =
//             left_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY).unwrap();
//         let stage = session
//             .create_reference_space(xr::ReferenceSpaceType::LOCAL_FLOOR, xr::Posef::IDENTITY).unwrap();

//         let views = xr_instance
//             .enumerate_view_configuration_views(xr_system_id, VIEW_TYPE)
//             .unwrap();
//         assert_eq!(views.len(), 2);
//         assert_eq!(views[0], views[1]);

//         Ok((
//             wgpu_util::Context::init_with_xr(wgpu_instance, wgpu_adapter, wgpu_device, wgpu_queue),
//             XrState {
//                 xr_instance,
//                 environment_blend_mode,
//                 session,
//                 session_running: false,
//                 frame_wait,
//                 frame_stream,

//                 action_set,
//                 right_action,
//                 left_action,
//                 right_thumbstick_action,
//                 right_space,
//                 left_space,
//                 stage,

//                 event_storage: xr::EventDataBuffer::new(),
//                 views,
//                 swapchain: None,
//             },
//         ))
//     }

//     pub fn pre_frame(&mut self) -> Result<Option<xr::FrameState>> {
//         while let Some(event) = self.xr_instance.poll_event(&mut self.event_storage).unwrap() {
//             use xr::Event::*;
//             match event {
//                 SessionStateChanged(e) => {
//                     // Session state change is where we can begin and end sessions, as well as
//                     // find quit messages!
//                     match e.state() {
//                         xr::SessionState::READY => {
//                             self.session.begin(VIEW_TYPE).unwrap();
//                             self.session_running = true;
//                         }
//                         xr::SessionState::STOPPING => {
//                             self.session.end().unwrap();
//                             self.session_running = false;
//                         }
//                         xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
//                             return Ok(None);
//                         }
//                         _ => {}
//                     }
//                 }
//                 InstanceLossPending(_) => {
//                     return Ok(None);
//                 }
//                 _ => {}
//             }
//         }

//         if !self.session_running {
//             // Don't grind up the CPU
//             std::thread::sleep(std::time::Duration::from_millis(10));
//             return Ok(None);
//         }

//         // Block until the previous frame is finished displaying, and is ready for another one.
//         // Also returns a prediction of when the next frame will be displayed, for use with
//         // predicting locations of controllers, viewpoints, etc.
//         let xr_frame_state = self.frame_wait.wait().unwrap();
//         // Must be called before any rendering is done!
//         self.frame_stream.begin().unwrap();

//         Ok(Some(xr_frame_state))
//     }

//     pub fn post_frame(
//         &mut self,
//         rt_texture_view: &wgpu::TextureView,
//         xr_frame_state: xr::FrameState,
//         device: &wgpu::Device,
//         command_encoder: &mut wgpu::CommandEncoder,
//         pipeline_database: &mut wgpu_util::PipelineDatabase,
//     ) -> Result<PostFrameData> {
//         use wgpu_hal::vulkan as V;

//         if !xr_frame_state.should_render {
//             self.frame_stream.end(
//                 xr_frame_state.predicted_display_time,
//                 self.environment_blend_mode,
//                 &[],
//             ).unwrap();
//             return Ok(PostFrameData::default());
//         }

//         // let swapchain = self.swapchain.get_or_insert_with(|| {
//         //     // Now we need to find all the viewpoints we need to take care of! This is a
//         //     // property of the view configuration type; in this example we use PRIMARY_STEREO,
//         //     // so we should have 2 viewpoints.

//         //     // Create a swapchain for the viewpoints! A swapchain is a set of texture buffers
//         //     // used for displaying to screen, typically this is a backbuffer and a front buffer,
//         //     // one for rendering data to, and one for displaying on-screen.
//         //     let resolution = vk::Extent2D {
//         //         width: self.views[0].recommended_image_rect_width,
//         //         height: self.views[0].recommended_image_rect_height,
//         //     };
//         //     let handle = self
//         //         .session
//         //         .create_swapchain(&xr::SwapchainCreateInfo {
//         //             create_flags: xr::SwapchainCreateFlags::EMPTY,
//         //             usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
//         //                 | xr::SwapchainUsageFlags::SAMPLED,
//         //             format: VK_COLOR_FORMAT.as_raw() as _,
//         //             // The Vulkan graphics pipeline we create is not set up for multisampling,
//         //             // so we hardcode this to 1. If we used a proper multisampling setup, we
//         //             // could set this to `views[0].recommended_swapchain_sample_count`.
//         //             sample_count: 1,
//         //             width: resolution.width,
//         //             height: resolution.height,
//         //             face_count: 1,
//         //             array_size: 2,
//         //             mip_count: 1,
//         //         })
//         //         .unwrap();

//         //     // We'll want to track our own information about the swapchain, so we can draw stuff
//         //     // onto it! We'll also create a buffer for each generated texture here as well.
//         //     let images = handle.enumerate_images().unwrap();

//         //     //let textures = vec![];
//         //     let mut texture_views = vec![];
//         //     for color_image in images {
//         //         let color_image = vk::Image::from_raw(color_image);
//         //         let wgpu_hal_texture = unsafe {
//         //             V::Device::texture_from_raw(
//         //                 color_image,
//         //                 &wgpu_hal::TextureDescriptor {
//         //                     label: Some("VR Swapchain"),
//         //                     size: wgpu::Extent3d {
//         //                         width: resolution.width,
//         //                         height: resolution.height,
//         //                         depth_or_array_layers: 2,
//         //                     },
//         //                     mip_level_count: 1,
//         //                     sample_count: 1,
//         //                     dimension: wgpu::TextureDimension::D2,
//         //                     format: WGPU_COLOR_FORMAT,
//         //                     usage: wgpu_hal::TextureUses::COLOR_TARGET
//         //                         | wgpu_hal::TextureUses::COPY_DST,
//         //                     memory_flags: wgpu_hal::MemoryFlags::empty(),
//         //                     view_formats: vec![],
//         //                 },
//         //                 None,
//         //             )
//         //         };
//         //         let texture = unsafe {
//         //             device.create_texture_from_hal::<wgpu_hal::api::Vulkan>(
//         //                 wgpu_hal_texture,
//         //                 &wgpu::TextureDescriptor {
//         //                     label: Some("VR Swapchain"),
//         //                     size: wgpu::Extent3d {
//         //                         width: resolution.width,
//         //                         height: resolution.height,
//         //                         depth_or_array_layers: 2,
//         //                     },
//         //                     mip_level_count: 1,
//         //                     sample_count: 1,
//         //                     dimension: wgpu::TextureDimension::D2,
//         //                     format: WGPU_COLOR_FORMAT,
//         //                     usage: wgpu::TextureUsages::RENDER_ATTACHMENT
//         //                         | wgpu::TextureUsages::COPY_DST,
//         //                     view_formats: &[],
//         //                 },
//         //             )
//         //         };
//         //         let view = texture.create_view(&wgpu::TextureViewDescriptor {
//         //             dimension: Some(wgpu::TextureViewDimension::D2Array),
//         //             array_layer_count: Some(2),
//         //             ..Default::default()
//         //         });

//         //         texture_views.push(view);
//         //     }

//         //     Swapchain {
//         //         handle,
//         //         resolution,
//         //         buffers: texture_views,
//         //     }
//         // });

//         self.session.sync_actions(&[(&self.action_set).into()]).unwrap();
//         let locate_hand_pose = |action: &xr::Action<xr::Posef>,
//                                 space: &xr::Space|
//          -> anyhow::Result<Option<(Vec3, Quat)>> {
//             if action.is_active(&self.session, xr::Path::NULL).unwrap() {
//                 Ok(Some(openxr_pose_to_glam(
//                     &space
//                         .locate(&self.stage, xr_frame_state.predicted_display_time).unwrap()
//                         .pose,
//                 )))
//             } else {
//                 Ok(None)
//             }
//         };

//         let left_hand = locate_hand_pose(&self.left_action, &self.left_space).unwrap();
//         let right_hand = locate_hand_pose(&self.right_action, &self.right_space).unwrap();

//         let right_thumbstick = if let Ok(value) =
//             xr::Vector2f::get(&self.right_thumbstick_action, &self.session, xr::Path::NULL)
//         {
//             Vec2::new(value.current_state.x, value.current_state.y)
//         } else {
//             Vec2::ZERO
//         };

//         let (_, views) = self.session.locate_views(
//             VIEW_TYPE,
//             xr_frame_state.predicted_display_time,
//             &self.stage,
//         ).unwrap();

//         // We need to ask which swapchain image to use for rendering! Which one will we get.unwrap()
//         // Who knows! It's up to the runtime to decide.
//         let image_index = swapchain.handle.acquire_image().unwrap();

//         // Wait until the image is available to render to. The compositor could still be
//         // reading from it.
//         swapchain.handle.wait_image(xr::Duration::INFINITE).unwrap();

//         blit_pass::encode(
//             &BlitPassParameters {
//                 src_view: rt_texture_view,
//                 dst_view: &swapchain.buffers[image_index as usize],
//                 multiview: Some(NonZeroU32::new(2).unwrap()),
//                 target_format: WGPU_COLOR_FORMAT,
//             },
//             device,
//             command_encoder,
//             pipeline_database,
//         );
//         // blit_state.encode_draw_pass(
//         //     encoder,
//         //     swapchain.buffers[image_index as usize].view(),
//         //     None,
//         // );

//         Ok(PostFrameData {
//             views,
//             left_hand,
//             right_hand,
//             right_thumbstick,
//         })
//     }

//     pub fn post_frame_submit(
//         &mut self,
//         xr_frame_state: xr::FrameState,
//         views: &[openxr::View],
//     ) -> anyhow::Result<()> {
//         if let Some(swapchain) = &mut self.swapchain {
//             swapchain.handle.release_image().unwrap();

//             let rect = xr::Rect2Di {
//                 offset: xr::Offset2Di { x: 0, y: 0 },
//                 extent: xr::Extent2Di {
//                     width: swapchain.resolution.width as _,
//                     height: swapchain.resolution.height as _,
//                 },
//             };

//             self.frame_stream.end(
//                 xr_frame_state.predicted_display_time,
//                 self.environment_blend_mode,
//                 &[&xr::CompositionLayerProjection::new()
//                     .space(&self.stage)
//                     .views(&[
//                         xr::CompositionLayerProjectionView::new()
//                             .pose(views[0].pose)
//                             .fov(views[0].fov)
//                             .sub_image(
//                                 xr::SwapchainSubImage::new()
//                                     .swapchain(&swapchain.handle)
//                                     .image_array_index(0)
//                                     .image_rect(rect),
//                             ),
//                         xr::CompositionLayerProjectionView::new()
//                             .pose(views[1].pose)
//                             .fov(views[1].fov)
//                             .sub_image(
//                                 xr::SwapchainSubImage::new()
//                                     .swapchain(&swapchain.handle)
//                                     .image_array_index(1)
//                                     .image_rect(rect),
//                             ),
//                     ])],
//             ).unwrap();
//         }

//         Ok(())
//     }
// }
