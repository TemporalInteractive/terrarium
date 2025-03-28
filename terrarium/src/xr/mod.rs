use std::{
    ffi::{c_void, CString},
    num::NonZeroU32,
};

use anyhow::{Context, Result};
use ash::vk::{self, Handle};
use glam::{Mat4, Quat, Vec2, Vec3};
use openxr::{self as xr, ActionInput, ViewConfigurationView};

use crate::{
    render_passes::blit_pass::{self, BlitPassParameters},
    wgpu_util,
};

const WGPU_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const VK_COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

#[derive(Default)]
pub struct PostFrameData {
    pub views: Vec<openxr::View>,
    pub left_hand: Option<(Vec3, Quat)>,
    pub right_hand: Option<(Vec3, Quat)>,
    pub right_thumbstick: Vec2,
}

pub fn openxr_pose_to_glam(pose: &openxr::Posef) -> (Vec3, Quat) {
    // with enough sign errors anything is possible
    let rotation = {
        let o = pose.orientation;
        Quat::from_rotation_x(180.0f32.to_radians()) * glam::quat(o.w, o.z, o.y, o.x)
    };
    let translation = glam::vec3(-pose.position.x, pose.position.y, -pose.position.z);
    (translation, rotation)
}

fn openxr_view_to_view_proj(v: &openxr::View, z_near: f32, z_far: f32) -> Mat4 {
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

struct Swapchain {
    handle: xr::Swapchain<xr::Vulkan>,
    resolution: vk::Extent2D,
    buffers: Vec<wgpu::TextureView>,
}

pub struct XrState {
    xr_instance: xr::Instance,
    environment_blend_mode: xr::EnvironmentBlendMode,
    session: xr::Session<xr::Vulkan>,
    session_running: bool,
    frame_wait: xr::FrameWaiter,
    frame_stream: xr::FrameStream<xr::Vulkan>,

    action_set: xr::ActionSet,
    right_action: xr::Action<xr::Posef>,
    left_action: xr::Action<xr::Posef>,
    right_thumbstick_action: xr::Action<xr::Vector2f>,
    right_space: xr::Space,
    left_space: xr::Space,
    stage: xr::Space,

    event_storage: xr::EventDataBuffer,
    views: Vec<openxr::ViewConfigurationView>,
    swapchain: Option<Swapchain>,
}

impl XrState {
    pub fn initialize_with_wgpu(
        wgpu_features: wgpu::Features,
        wgpu_limits: wgpu::Limits,
    ) -> anyhow::Result<(wgpu_util::Context, XrState)> {
        use wgpu_hal::vulkan as V;

        let entry = xr::Entry::linked();

        let available_extensions = entry.enumerate_extensions()?;
        assert!(available_extensions.khr_vulkan_enable2);
        println!("available xr exts: {:#?}", available_extensions);

        let mut enabled_extensions = xr::ExtensionSet::default();
        enabled_extensions.khr_vulkan_enable2 = true;
        #[cfg(target_os = "android")]
        {
            enabled_extensions.khr_android_create_instance = true;
        }

        let available_layers = entry.enumerate_layers()?;
        println!("available xr layers: {:#?}", available_layers);

        let xr_instance = entry.create_instance(
            &xr::ApplicationInfo {
                application_name: "terrarium",
                ..Default::default()
            },
            &enabled_extensions,
            &[],
        )?;
        let instance_props = xr_instance.properties()?;
        let xr_system_id = xr_instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
        let system_props = xr_instance.system_properties(xr_system_id).unwrap();
        println!(
            "loaded OpenXR runtime: {} {} {}",
            instance_props.runtime_name,
            instance_props.runtime_version,
            if system_props.system_name.is_empty() {
                "<unnamed>"
            } else {
                &system_props.system_name
            }
        );

        let environment_blend_mode =
            xr_instance.enumerate_environment_blend_modes(xr_system_id, VIEW_TYPE)?[0];
        let vk_target_version = vk::make_api_version(0, 1, 1, 0);
        let vk_target_version_xr = xr::Version::new(1, 1, 0);
        let reqs = xr_instance.graphics_requirements::<xr::Vulkan>(xr_system_id)?;
        if vk_target_version_xr < reqs.min_api_version_supported
            || vk_target_version_xr.major() > reqs.max_api_version_supported.major()
        {
            panic!(
                "OpenXR runtime requires Vulkan version > {}, < {}.0.0",
                reqs.min_api_version_supported,
                reqs.max_api_version_supported.major() + 1
            );
        }

        let vk_entry = unsafe { ash::Entry::load() }?;

        let flags = wgpu::InstanceFlags::empty();
        let mut extensions = V::Instance::desired_extensions(&vk_entry, vk_target_version, flags)?;
        extensions.push(ash::khr::swapchain::NAME);
        println!(
            "creating vulkan instance with these extensions: {:#?}",
            extensions
        );

        let vk_instance = unsafe {
            //let extensions_cchar: Vec<_> = extensions.iter().map(|s| s.as_ptr()).collect();

            let app_name = CString::new("wgpu-openxr-example")?;
            let vk_app_info = vk::ApplicationInfo::default()
                .application_name(&app_name)
                .application_version(1)
                .engine_name(&app_name)
                .engine_version(1)
                .api_version(vk_target_version);

            // let vk_instance = xr_instance
            //     .create_vulkan_instance(
            //         xr_system_id,
            //         std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
            //         &vk::InstanceCreateInfo::builder()
            //             .application_info(&vk_app_info)
            //             .enabled_extension_names(&extensions_cchar) as *const _
            //             as *const _,
            //     )
            //     .context("XR error creating Vulkan instance")?
            //     .map_err(vk::Result::from_raw)
            //     .context("Vulkan error creating Vulkan instance")?;

            let vk_instance = xr_instance
                .create_vulkan_instance(
                    xr_system_id,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    &vk::InstanceCreateInfo::default().application_info(&vk_app_info) as *const _
                        as *const _,
                )
                .expect("XR error creating Vulkan instance")
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan instance");

            ash::Instance::load(
                vk_entry.static_fn(),
                vk::Instance::from_raw(vk_instance as _),
            )
        };
        println!("created vulkan instance");

        let vk_instance_ptr = vk_instance.handle().as_raw() as *const c_void;

        let vk_physical_device = vk::PhysicalDevice::from_raw(unsafe {
            xr_instance.vulkan_graphics_device(xr_system_id, vk_instance.handle().as_raw() as _)?
                as _
        });
        let vk_physical_device_ptr = vk_physical_device.as_raw() as *const c_void;

        let vk_device_properties =
            unsafe { vk_instance.get_physical_device_properties(vk_physical_device) };
        if vk_device_properties.api_version < vk_target_version {
            unsafe { vk_instance.destroy_instance(None) }
            panic!("Vulkan physical device doesn't support version 1.1");
        }

        //wgpu_hal::vulkan::Instance::from_raw(entry, raw_instance, instance_api_version, android_sdk_version, debug_utils_create_info, extensions, flags, has_nv_optimus, drop_callback)

        let wgpu_vk_instance = unsafe {
            V::Instance::from_raw(
                vk_entry.clone(),
                vk_instance.clone(),
                vk_target_version,
                0,
                None,
                extensions,
                flags,
                false,
                Some(Box::new(|| ())), //?
            )?
        };
        let wgpu_exposed_adapter = wgpu_vk_instance
            .expose_adapter(vk_physical_device)
            .context("failed to expose adapter")?;

        let enabled_extensions = wgpu_exposed_adapter
            .adapter
            .required_device_extensions(wgpu_features);

        let (wgpu_open_device, vk_device_ptr, queue_family_index) = {
            // let uab_types = wgpu_hal::UpdateAfterBindTypes::from_limits(
            //     &wgpu_limits,
            //     &wgpu_exposed_adapter
            //         .adapter
            //         .physical_device_capabilities()
            //         .properties()
            //         .limits,
            // );

            let mut enabled_phd_features = wgpu_exposed_adapter
                .adapter
                .physical_device_features(&enabled_extensions, wgpu_features);
            let family_index = 0;
            let family_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family_index)
                .queue_priorities(&[1.0]);
            let family_infos = [family_info];
            let mut multi_view_features = vk::PhysicalDeviceMultiviewFeatures {
                multiview: vk::TRUE,
                ..Default::default()
            };
            let info = enabled_phd_features.add_to_device_create(
                vk::DeviceCreateInfo::default()
                    .queue_create_infos(&family_infos)
                    .push_next(&mut multi_view_features),
            );
            let vk_device = unsafe {
                let vk_device = xr_instance
                    .create_vulkan_device(
                        xr_system_id,
                        std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                        vk_physical_device.as_raw() as _,
                        &info as *const _ as *const _,
                    )
                    .context("XR error creating Vulkan device")?
                    .map_err(vk::Result::from_raw)
                    .context("Vulkan error creating Vulkan device")?;

                ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(vk_device as _))
            };
            let vk_device_ptr = vk_device.handle().as_raw() as *const c_void;

            let wgpu_open_device: wgpu_hal::OpenDevice<V::Api> = unsafe {
                wgpu_exposed_adapter.adapter.device_from_raw(
                    vk_device,
                    Some(Box::new(|| ())), // ?
                    &enabled_extensions,
                    wgpu_features,
                    &wgpu::MemoryHints::Performance,
                    family_info.queue_family_index,
                    0,
                )
            }?;

            (
                wgpu_open_device,
                vk_device_ptr,
                family_info.queue_family_index,
            )
        };

        let wgpu_instance =
            unsafe { wgpu::Instance::from_hal::<wgpu_hal::api::Vulkan>(wgpu_vk_instance) };
        let wgpu_adapter = unsafe { wgpu_instance.create_adapter_from_hal(wgpu_exposed_adapter) };
        let (wgpu_device, wgpu_queue) = unsafe {
            wgpu_adapter.create_device_from_hal(
                wgpu_open_device,
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu_features,
                    required_limits: wgpu_limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
        }?;

        let (session, frame_wait, frame_stream) = unsafe {
            xr_instance.create_session::<xr::Vulkan>(
                xr_system_id,
                &xr::vulkan::SessionCreateInfo {
                    instance: vk_instance_ptr,
                    physical_device: vk_physical_device_ptr,
                    device: vk_device_ptr,
                    queue_family_index,
                    queue_index: 0,
                },
            )
        }?;
        let action_set = xr_instance.create_action_set("input", "input pose information", 0)?;
        let right_action =
            action_set.create_action::<xr::Posef>("right_hand", "Right Hand Controller", &[])?;
        let left_action =
            action_set.create_action::<xr::Posef>("left_hand", "Left Hand Controller", &[])?;
        let right_thumbstick_action = action_set.create_action::<xr::Vector2f>(
            "right_thumbstick",
            "Right Hand Controller Thumbstick",
            &[],
        )?;
        xr_instance.suggest_interaction_profile_bindings(
            xr_instance.string_to_path("/interaction_profiles/oculus/touch_controller")?, // /interaction_profiles/khr/simple_controller
            &[
                xr::Binding::new(
                    &right_action,
                    xr_instance.string_to_path("/user/hand/right/input/grip/pose")?,
                ),
                xr::Binding::new(
                    &left_action,
                    xr_instance.string_to_path("/user/hand/left/input/grip/pose")?,
                ),
                xr::Binding::new(
                    &right_thumbstick_action,
                    xr_instance.string_to_path("/user/hand/left/input/thumbstick")?,
                ),
            ],
        )?;
        session.attach_action_sets(&[&action_set])?;
        let right_space =
            right_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?;
        let left_space =
            left_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?;
        let stage =
            session.create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;

        let views = xr_instance
            .enumerate_view_configuration_views(xr_system_id, VIEW_TYPE)
            .unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0], views[1]);

        Ok((
            wgpu_util::Context::init_with_xr(wgpu_instance, wgpu_adapter, wgpu_device, wgpu_queue),
            XrState {
                xr_instance,
                environment_blend_mode,
                session,
                session_running: false,
                frame_wait,
                frame_stream,

                action_set,
                right_action,
                left_action,
                right_thumbstick_action,
                right_space,
                left_space,
                stage,

                event_storage: xr::EventDataBuffer::new(),
                views,
                swapchain: None,
            },
        ))
    }

    pub fn pre_frame(&mut self) -> Result<Option<xr::FrameState>> {
        while let Some(event) = self.xr_instance.poll_event(&mut self.event_storage)? {
            use xr::Event::*;
            match event {
                SessionStateChanged(e) => {
                    // Session state change is where we can begin and end sessions, as well as
                    // find quit messages!
                    match e.state() {
                        xr::SessionState::READY => {
                            self.session.begin(VIEW_TYPE)?;
                            self.session_running = true;
                        }
                        xr::SessionState::STOPPING => {
                            self.session.end()?;
                            self.session_running = false;
                        }
                        xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                            return Ok(None);
                        }
                        _ => {}
                    }
                }
                InstanceLossPending(_) => {
                    return Ok(None);
                }
                _ => {}
            }
        }

        if !self.session_running {
            // Don't grind up the CPU
            std::thread::sleep(std::time::Duration::from_millis(10));
            return Ok(None);
        }

        // Block until the previous frame is finished displaying, and is ready for another one.
        // Also returns a prediction of when the next frame will be displayed, for use with
        // predicting locations of controllers, viewpoints, etc.
        let xr_frame_state = self.frame_wait.wait()?;
        // Must be called before any rendering is done!
        self.frame_stream.begin()?;

        Ok(Some(xr_frame_state))
    }

    pub fn post_frame(
        &mut self,
        rt_texture_view: &wgpu::TextureView,
        xr_camera_buffer: &wgpu::Buffer,
        xr_frame_state: xr::FrameState,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        command_encoder: &mut wgpu::CommandEncoder,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) -> Result<PostFrameData> {
        use wgpu_hal::vulkan as V;

        if !xr_frame_state.should_render {
            self.frame_stream.end(
                xr_frame_state.predicted_display_time,
                self.environment_blend_mode,
                &[],
            )?;
            return Ok(PostFrameData::default());
        }

        let swapchain = self.swapchain.get_or_insert_with(|| {
            // Now we need to find all the viewpoints we need to take care of! This is a
            // property of the view configuration type; in this example we use PRIMARY_STEREO,
            // so we should have 2 viewpoints.

            // Create a swapchain for the viewpoints! A swapchain is a set of texture buffers
            // used for displaying to screen, typically this is a backbuffer and a front buffer,
            // one for rendering data to, and one for displaying on-screen.
            let resolution = vk::Extent2D {
                width: self.views[0].recommended_image_rect_width,
                height: self.views[0].recommended_image_rect_height,
            };
            let handle = self
                .session
                .create_swapchain(&xr::SwapchainCreateInfo {
                    create_flags: xr::SwapchainCreateFlags::EMPTY,
                    usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                        | xr::SwapchainUsageFlags::SAMPLED,
                    format: VK_COLOR_FORMAT.as_raw() as _,
                    // The Vulkan graphics pipeline we create is not set up for multisampling,
                    // so we hardcode this to 1. If we used a proper multisampling setup, we
                    // could set this to `views[0].recommended_swapchain_sample_count`.
                    sample_count: 1,
                    width: resolution.width,
                    height: resolution.height,
                    face_count: 1,
                    array_size: 2,
                    mip_count: 1,
                })
                .unwrap();

            // We'll want to track our own information about the swapchain, so we can draw stuff
            // onto it! We'll also create a buffer for each generated texture here as well.
            let images = handle.enumerate_images().unwrap();

            //let textures = vec![];
            let mut texture_views = vec![];
            for color_image in images {
                let color_image = vk::Image::from_raw(color_image);
                let wgpu_hal_texture = unsafe {
                    V::Device::texture_from_raw(
                        color_image,
                        &wgpu_hal::TextureDescriptor {
                            label: Some("VR Swapchain"),
                            size: wgpu::Extent3d {
                                width: resolution.width,
                                height: resolution.height,
                                depth_or_array_layers: 2,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: WGPU_COLOR_FORMAT,
                            usage: wgpu_hal::TextureUses::COLOR_TARGET
                                | wgpu_hal::TextureUses::COPY_DST,
                            memory_flags: wgpu_hal::MemoryFlags::empty(),
                            view_formats: vec![],
                        },
                        None,
                    )
                };
                let texture = unsafe {
                    device.create_texture_from_hal::<wgpu_hal::api::Vulkan>(
                        wgpu_hal_texture,
                        &wgpu::TextureDescriptor {
                            label: Some("VR Swapchain"),
                            size: wgpu::Extent3d {
                                width: resolution.width,
                                height: resolution.height,
                                depth_or_array_layers: 2,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: WGPU_COLOR_FORMAT,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::COPY_DST,
                            view_formats: &[],
                        },
                    )
                };
                let view = texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    array_layer_count: Some(2),
                    ..Default::default()
                });

                texture_views.push(view);
            }

            Swapchain {
                handle,
                resolution,
                buffers: texture_views,
            }
        });

        self.session.sync_actions(&[(&self.action_set).into()])?;
        let locate_hand_pose = |action: &xr::Action<xr::Posef>,
                                space: &xr::Space|
         -> anyhow::Result<Option<(Vec3, Quat)>> {
            if action.is_active(&self.session, xr::Path::NULL)? {
                Ok(Some(openxr_pose_to_glam(
                    &space
                        .locate(&self.stage, xr_frame_state.predicted_display_time)?
                        .pose,
                )))
            } else {
                Ok(None)
            }
        };

        let left_hand = locate_hand_pose(&self.left_action, &self.left_space)?;
        let right_hand = locate_hand_pose(&self.right_action, &self.right_space)?;

        let right_thumbstick = if let Ok(value) =
            xr::Vector2f::get(&self.right_thumbstick_action, &self.session, xr::Path::NULL)
        {
            Vec2::new(value.current_state.x, value.current_state.y)
        } else {
            Vec2::ZERO
        };

        let (_, views) = self.session.locate_views(
            VIEW_TYPE,
            xr_frame_state.predicted_display_time,
            &self.stage,
        )?;

        let mut view_proj_matrices = [Mat4::IDENTITY; 2];
        for i in 0..2 {
            view_proj_matrices[i] = openxr_view_to_view_proj(&views[i], 0.01, 1000.0);
        }
        queue.write_buffer(xr_camera_buffer, 0, bytemuck::bytes_of(&view_proj_matrices));

        // We need to ask which swapchain image to use for rendering! Which one will we get?
        // Who knows! It's up to the runtime to decide.
        let image_index = swapchain.handle.acquire_image().unwrap();

        // Wait until the image is available to render to. The compositor could still be
        // reading from it.
        swapchain.handle.wait_image(xr::Duration::INFINITE).unwrap();

        blit_pass::encode(
            &BlitPassParameters {
                src_view: rt_texture_view,
                dst_view: &swapchain.buffers[image_index as usize],
                multiview: Some(NonZeroU32::new(2).unwrap()),
                target_format: WGPU_COLOR_FORMAT,
            },
            device,
            command_encoder,
            pipeline_database,
        );
        // blit_state.encode_draw_pass(
        //     encoder,
        //     swapchain.buffers[image_index as usize].view(),
        //     None,
        // );

        Ok(PostFrameData {
            views,
            left_hand,
            right_hand,
            right_thumbstick,
        })
    }

    pub fn post_frame_submit(
        &mut self,
        xr_frame_state: xr::FrameState,
        views: &[openxr::View],
    ) -> anyhow::Result<()> {
        if let Some(swapchain) = &mut self.swapchain {
            swapchain.handle.release_image().unwrap();

            let rect = xr::Rect2Di {
                offset: xr::Offset2Di { x: 0, y: 0 },
                extent: xr::Extent2Di {
                    width: swapchain.resolution.width as _,
                    height: swapchain.resolution.height as _,
                },
            };

            self.frame_stream.end(
                xr_frame_state.predicted_display_time,
                self.environment_blend_mode,
                &[&xr::CompositionLayerProjection::new()
                    .space(&self.stage)
                    .views(&[
                        xr::CompositionLayerProjectionView::new()
                            .pose(views[0].pose)
                            .fov(views[0].fov)
                            .sub_image(
                                xr::SwapchainSubImage::new()
                                    .swapchain(&swapchain.handle)
                                    .image_array_index(0)
                                    .image_rect(rect),
                            ),
                        xr::CompositionLayerProjectionView::new()
                            .pose(views[1].pose)
                            .fov(views[1].fov)
                            .sub_image(
                                xr::SwapchainSubImage::new()
                                    .swapchain(&swapchain.handle)
                                    .image_array_index(1)
                                    .image_rect(rect),
                            ),
                    ])],
            )?;
        }

        Ok(())
    }
}
