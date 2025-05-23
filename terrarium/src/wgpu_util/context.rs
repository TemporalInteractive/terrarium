#![allow(clippy::missing_transmute_annotations)]

use anyhow::Result;
use ash::vk::{self, Handle};
use std::ffi::{c_void, CString};
use std::num::NonZeroU32;
use std::sync::Arc;
use wgpu::{DownlevelCapabilities, Features, Limits, PowerPreference};
use winit::window::Window;

use super::surface::Surface;
use crate::render_passes::blit_pass::{self, BlitPassParameters};
use crate::{wgpu_util, xr};

struct XrSwapchain {
    handle: openxr::Swapchain<openxr::Vulkan>,
    resolution: vk::Extent2D,
    buffers: Vec<wgpu::TextureView>,
}

pub struct XrContext {
    pub instance: openxr::Instance,
    pub session: openxr::Session<openxr::Vulkan>,
    session_running: bool,
    pub environment_blend_mode: openxr::EnvironmentBlendMode,
    pub frame_wait: openxr::FrameWaiter,
    pub frame_stream: openxr::FrameStream<openxr::Vulkan>,
    event_storage: openxr::EventDataBuffer,
    pub view_configs: Vec<openxr::ViewConfigurationView>,
    pub stage: openxr::Space,
    swapchain: Option<XrSwapchain>,
}

pub struct Context {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub xr: Option<XrContext>,
}

impl Context {
    pub(crate) async fn init_with_window(
        surface: &mut Surface,
        window: Arc<Window>,
        optional_features: Features,
        required_features: Features,
        required_downlevel_capabilities: DownlevelCapabilities,
        required_limits: Limits,
        no_gpu_validation: bool,
    ) -> Self {
        let mut flags = wgpu::InstanceFlags::DEBUG;
        if !no_gpu_validation {
            flags |= wgpu::InstanceFlags::VALIDATION;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags,
            backend_options: wgpu::BackendOptions::default(),
        });
        surface.pre_adapter(&instance, window);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Failed to find suitable GPU adapter.");

        let adapter_features = adapter.features();
        assert!(
            adapter_features.contains(required_features),
            "Adapter does not support required features for this example: {:?}",
            required_features - adapter_features
        );

        let downlevel_capabilities = adapter.get_downlevel_capabilities();
        assert!(
            downlevel_capabilities.shader_model >= required_downlevel_capabilities.shader_model,
            "Adapter does not support the minimum shader model required to run this example: {:?}",
            required_downlevel_capabilities.shader_model
        );
        assert!(
        downlevel_capabilities
            .flags
            .contains(required_downlevel_capabilities.flags),
        "Adapter does not support the downlevel capabilities required to run this example: {:?}",
        required_downlevel_capabilities.flags - downlevel_capabilities.flags
    );

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: (optional_features & adapter_features) | required_features,
                required_limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Unable to find a suitable GPU adapter!");

        Self {
            instance,
            adapter,
            device,
            queue,
            xr: None,
        }
    }

    pub(crate) fn init_with_xr(
        required_features: wgpu::Features,
        required_limits: wgpu::Limits,
        no_gpu_validation: bool,
    ) -> Result<Self> {
        use anyhow::Context;
        use wgpu_hal::vulkan as V;

        let entry = openxr::Entry::linked();

        let available_extensions = entry.enumerate_extensions()?;
        assert!(available_extensions.khr_vulkan_enable2);
        println!("available openxr exts: {:#?}", available_extensions);

        let mut enabled_extensions = openxr::ExtensionSet::default();
        enabled_extensions.khr_vulkan_enable2 = true;
        #[cfg(target_os = "android")]
        {
            enabled_extensions.khr_android_create_instance = true;
        }

        let available_layers = entry.enumerate_layers()?;
        println!("available openxr layers: {:#?}", available_layers);

        let xr_instance = entry.create_instance(
            &openxr::ApplicationInfo {
                application_name: "terrarium",
                ..Default::default()
            },
            &enabled_extensions,
            &[],
        )?;
        let instance_props = xr_instance.properties()?;
        let xr_system_id = xr_instance.system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
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
            xr_instance.enumerate_environment_blend_modes(xr_system_id, xr::VIEW_TYPE)?[0];
        let vk_target_version = vk::make_api_version(0, 1, 1, 0);
        let vk_target_version_xr = openxr::Version::new(1, 1, 0);
        let reqs = xr_instance.graphics_requirements::<openxr::Vulkan>(xr_system_id)?;
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

        let mut flags = wgpu::InstanceFlags::DEBUG;
        if !no_gpu_validation {
            flags |= wgpu::InstanceFlags::VALIDATION;
        }
        let extensions = V::Instance::desired_extensions(&vk_entry, vk_target_version, flags)?;
        let extensions_cchar: Vec<_> = extensions.iter().map(|s| s.as_ptr()).collect();

        let vk_instance = unsafe {
            let app_name = CString::new("wgpu-openxr-example")?;
            let vk_app_info = vk::ApplicationInfo::default()
                .application_name(&app_name)
                .application_version(1)
                .engine_name(&app_name)
                .engine_version(1)
                .api_version(vk_target_version);

            let vk_instance = xr_instance
                .create_vulkan_instance(
                    xr_system_id,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    &vk::InstanceCreateInfo::default()
                        .application_info(&vk_app_info)
                        .enabled_extension_names(&extensions_cchar) as *const _
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
            .required_device_extensions(required_features);

        let (wgpu_open_device, vk_device_ptr, queue_family_index) = {
            let mut enabled_phd_features = wgpu_exposed_adapter
                .adapter
                .physical_device_features(&enabled_extensions, required_features);
            let family_index = 0;
            let family_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family_index)
                .queue_priorities(&[1.0]);
            let family_infos = [family_info];
            let mut multi_view_features = vk::PhysicalDeviceMultiviewFeatures {
                multiview: vk::TRUE,
                ..Default::default()
            };

            // TODO: derive from gpu features
            let device_extensions = [
                c"VK_KHR_swapchain",
                c"VK_KHR_acceleration_structure",
                c"VK_KHR_ray_query",
                c"VK_KHR_buffer_device_address",
            ];
            let device_extensions_cchar: Vec<_> =
                device_extensions.iter().map(|s| s.as_ptr()).collect();

            let info = enabled_phd_features.add_to_device_create(
                vk::DeviceCreateInfo::default()
                    .queue_create_infos(&family_infos)
                    .enabled_extension_names(&device_extensions_cchar)
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
                    Some(Box::new(|| ())),
                    &enabled_extensions,
                    required_features,
                    &wgpu::MemoryHints::MemoryUsage,
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
                    required_features,
                    required_limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                },
            )
        }?;

        let (session, frame_wait, frame_stream) = unsafe {
            xr_instance.create_session::<openxr::Vulkan>(
                xr_system_id,
                &openxr::vulkan::SessionCreateInfo {
                    instance: vk_instance_ptr,
                    physical_device: vk_physical_device_ptr,
                    device: vk_device_ptr,
                    queue_family_index,
                    queue_index: 0,
                },
            )
        }?;

        let view_configs = xr_instance
            .enumerate_view_configuration_views(xr_system_id, xr::VIEW_TYPE)
            .unwrap();
        assert_eq!(view_configs.len(), 2);
        assert_eq!(view_configs[0], view_configs[1]);

        let stage = session
            .create_reference_space(openxr::ReferenceSpaceType::STAGE, openxr::Posef::IDENTITY)?;

        let xr = Some(XrContext {
            instance: xr_instance,
            session,
            session_running: false,
            environment_blend_mode,
            frame_wait,
            frame_stream,
            event_storage: openxr::EventDataBuffer::new(),
            view_configs,
            stage,
            swapchain: None,
        });

        Ok(Self {
            instance: wgpu_instance,
            adapter: wgpu_adapter,
            device: wgpu_device,
            queue: wgpu_queue,
            xr,
        })
    }
}

impl XrContext {
    pub(crate) fn pre_frame(&mut self) -> Result<Option<openxr::FrameState>> {
        while let Some(event) = self.instance.poll_event(&mut self.event_storage)? {
            use openxr::Event::*;
            match event {
                SessionStateChanged(e) => {
                    // Session state change is where we can begin and end sessions, as well as
                    // find quit messages!
                    match e.state() {
                        openxr::SessionState::READY => {
                            self.session.begin(xr::VIEW_TYPE)?;
                            self.session_running = true;
                        }
                        openxr::SessionState::STOPPING => {
                            self.session.end()?;
                            self.session_running = false;
                        }
                        openxr::SessionState::EXITING | openxr::SessionState::LOSS_PENDING => {
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

        Ok(Some(xr_frame_state))
    }

    pub(crate) fn pre_render(&mut self) -> anyhow::Result<()> {
        // Must be called before any rendering is done!
        self.frame_stream.begin()?;

        Ok(())
    }

    pub(crate) fn post_frame(
        &mut self,
        rt_texture_view: &wgpu::TextureView,
        xr_frame_state: openxr::FrameState,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) -> Result<Vec<openxr::View>> {
        if !xr_frame_state.should_render {
            self.frame_stream.end(
                xr_frame_state.predicted_display_time,
                self.environment_blend_mode,
                &[],
            )?;
            return Ok(vec![]);
        }

        let (_, views) = self.session.locate_views(
            xr::VIEW_TYPE,
            xr_frame_state.predicted_display_time,
            &self.stage,
        )?;

        let swapchain = self.get_swapchain(device).unwrap();

        // We need to ask which swapchain image to use for rendering! Which one will we get?
        // Who knows! It's up to the runtime to decide.
        let image_index = swapchain.handle.acquire_image().unwrap();

        // Wait until the image is available to render to. The compositor could still be
        // reading from it.
        swapchain
            .handle
            .wait_image(openxr::Duration::INFINITE)
            .unwrap();

        blit_pass::encode(
            &BlitPassParameters {
                src_view: rt_texture_view,
                dst_view: &swapchain.buffers[image_index as usize],
                multiview: Some(NonZeroU32::new(2).unwrap()),
                view_index_override: None,
                target_format: xr::WGPU_COLOR_FORMAT,
            },
            device,
            command_encoder,
            pipeline_database,
        );

        Ok(views)
    }

    pub(crate) fn post_frame_submit(
        &mut self,
        xr_frame_state: openxr::FrameState,
        views: &[openxr::View],
    ) -> Result<()> {
        if xr_frame_state.should_render {
            if let Some(swapchain) = &mut self.swapchain {
                swapchain.handle.release_image().unwrap();

                let rect = openxr::Rect2Di {
                    offset: openxr::Offset2Di { x: 0, y: 0 },
                    extent: openxr::Extent2Di {
                        width: swapchain.resolution.width as _,
                        height: swapchain.resolution.height as _,
                    },
                };

                self.frame_stream.end(
                    xr_frame_state.predicted_display_time,
                    self.environment_blend_mode,
                    &[&openxr::CompositionLayerProjection::new()
                        .space(&self.stage)
                        .views(&[
                            openxr::CompositionLayerProjectionView::new()
                                .pose(views[0].pose)
                                .fov(views[0].fov)
                                .sub_image(
                                    openxr::SwapchainSubImage::new()
                                        .swapchain(&swapchain.handle)
                                        .image_array_index(0)
                                        .image_rect(rect),
                                ),
                            openxr::CompositionLayerProjectionView::new()
                                .pose(views[1].pose)
                                .fov(views[1].fov)
                                .sub_image(
                                    openxr::SwapchainSubImage::new()
                                        .swapchain(&swapchain.handle)
                                        .image_array_index(1)
                                        .image_rect(rect),
                                ),
                        ])],
                )?;
            }
        }

        Ok(())
    }

    fn get_swapchain(&mut self, device: &wgpu::Device) -> Option<&mut XrSwapchain> {
        Some(self.swapchain.get_or_insert_with(|| {
            // Now we need to find all the viewpoints we need to take care of! This is a
            // property of the view configuration type; in this example we use PRIMARY_STEREO,
            // so we should have 2 viewpoints.

            // Create a swapchain for the viewpoints! A swapchain is a set of texture buffers
            // used for displaying to screen, typically this is a backbuffer and a front buffer,
            // one for rendering data to, and one for displaying on-screen.
            let resolution = vk::Extent2D {
                width: self.view_configs[0].recommended_image_rect_width,
                height: self.view_configs[0].recommended_image_rect_height,
            };
            let handle = self
                .session
                .create_swapchain(&openxr::SwapchainCreateInfo {
                    create_flags: openxr::SwapchainCreateFlags::EMPTY,
                    usage_flags: openxr::SwapchainUsageFlags::COLOR_ATTACHMENT
                        | openxr::SwapchainUsageFlags::SAMPLED,
                    format: xr::VK_COLOR_FORMAT.as_raw() as _,
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

            let mut texture_views = vec![];
            for color_image in images {
                let color_image = vk::Image::from_raw(color_image);
                let wgpu_hal_texture = unsafe {
                    wgpu_hal::vulkan::Device::texture_from_raw(
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
                            format: xr::WGPU_COLOR_FORMAT,
                            usage: wgpu::TextureUses::COLOR_TARGET | wgpu::TextureUses::COPY_DST,
                            memory_flags: wgpu_hal::MemoryFlags::empty(),
                            view_formats: vec![],
                        },
                        Some(Box::new(|| ())),
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
                            format: xr::WGPU_COLOR_FORMAT,
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

            XrSwapchain {
                handle,
                resolution,
                buffers: texture_views,
            }
        }))
    }
}
