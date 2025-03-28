use std::{
    ffi::{c_void, CString},
    num::NonZeroU32,
};

use anyhow::{Context, Result};
use ash::vk::{self, Handle};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec2, Vec3};
use openxr::{self as xr, ActionInput, ViewConfigurationView};

use crate::{
    render_passes::blit_pass::{self, BlitPassParameters},
    wgpu_util,
};

pub const WGPU_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const VK_COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

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

// #[derive(Default)]
// pub struct PostFrameData {
//     pub views: Vec<openxr::View>,
//     pub left_hand: Option<(Vec3, Quat)>,
//     pub right_hand: Option<(Vec3, Quat)>,
//     pub right_thumbstick: Vec2,
// }

pub fn openxr_pose_to_glam(pose: &openxr::Posef) -> (Vec3, Quat) {
    // with enough sign errors anything is possible
    let rotation = {
        let o = pose.orientation;
        Quat::from_rotation_x(180.0f32.to_radians()) * glam::quat(o.w, o.z, o.y, o.x)
    };
    let translation = glam::vec3(-pose.position.x, pose.position.y, -pose.position.z);
    (translation, rotation)
}

pub fn openxr_view_to_view_proj(v: &openxr::View, z_near: f32, z_far: f32) -> Mat4 {
    let pose = v.pose;
    let (xr_translation, xr_rotation) = openxr_pose_to_glam(&pose);

    let view = Mat4::look_at_rh(
        xr_translation,
        xr_translation + xr_rotation * Vec3::Z,
        xr_rotation * Vec3::Y,
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
//         let action_set = xr_instance.create_action_set("input", "input pose information", 0)?;
//         let right_action =
//             action_set.create_action::<xr::Posef>("right_hand", "Right Hand Controller", &[])?;
//         let left_action =
//             action_set.create_action::<xr::Posef>("left_hand", "Left Hand Controller", &[])?;
//         let right_thumbstick_action = action_set.create_action::<xr::Vector2f>(
//             "right_thumbstick",
//             "Right Hand Controller Thumbstick",
//             &[],
//         )?;
//         xr_instance.suggest_interaction_profile_bindings(
//             xr_instance.string_to_path("/interaction_profiles/oculus/touch_controller")?, // /interaction_profiles/khr/simple_controller
//             &[
//                 xr::Binding::new(
//                     &right_action,
//                     xr_instance.string_to_path("/user/hand/right/input/grip/pose")?,
//                 ),
//                 xr::Binding::new(
//                     &left_action,
//                     xr_instance.string_to_path("/user/hand/left/input/grip/pose")?,
//                 ),
//                 xr::Binding::new(
//                     &right_thumbstick_action,
//                     xr_instance.string_to_path("/user/hand/left/input/thumbstick")?,
//                 ),
//             ],
//         )?;
//         session.attach_action_sets(&[&action_set])?;
//         let right_space =
//             right_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?;
//         let left_space =
//             left_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?;
//         let stage = session
//             .create_reference_space(xr::ReferenceSpaceType::LOCAL_FLOOR, xr::Posef::IDENTITY)?;

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
//         while let Some(event) = self.xr_instance.poll_event(&mut self.event_storage)? {
//             use xr::Event::*;
//             match event {
//                 SessionStateChanged(e) => {
//                     // Session state change is where we can begin and end sessions, as well as
//                     // find quit messages!
//                     match e.state() {
//                         xr::SessionState::READY => {
//                             self.session.begin(VIEW_TYPE)?;
//                             self.session_running = true;
//                         }
//                         xr::SessionState::STOPPING => {
//                             self.session.end()?;
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
//         let xr_frame_state = self.frame_wait.wait()?;
//         // Must be called before any rendering is done!
//         self.frame_stream.begin()?;

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
//             )?;
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

//         self.session.sync_actions(&[(&self.action_set).into()])?;
//         let locate_hand_pose = |action: &xr::Action<xr::Posef>,
//                                 space: &xr::Space|
//          -> anyhow::Result<Option<(Vec3, Quat)>> {
//             if action.is_active(&self.session, xr::Path::NULL)? {
//                 Ok(Some(openxr_pose_to_glam(
//                     &space
//                         .locate(&self.stage, xr_frame_state.predicted_display_time)?
//                         .pose,
//                 )))
//             } else {
//                 Ok(None)
//             }
//         };

//         let left_hand = locate_hand_pose(&self.left_action, &self.left_space)?;
//         let right_hand = locate_hand_pose(&self.right_action, &self.right_space)?;

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
//         )?;

//         // We need to ask which swapchain image to use for rendering! Which one will we get?
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
//             )?;
//         }

//         Ok(())
//     }
// }
