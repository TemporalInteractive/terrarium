use std::{
    ffi::{c_void, CString},
    num::NonZeroU32,
};

use anyhow::Context;
use ash::vk::{self, Handle};
use glam::{Quat, Vec2, Vec3};
use openxr::{self as xr, ActionInput, ViewConfigurationView};

use crate::wgpu_util;

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

struct Swapchain {
    handle: xr::Swapchain<xr::Vulkan>,
    resolution: vk::Extent2D,
    buffers: Vec<wgpu::Texture>,
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
}
