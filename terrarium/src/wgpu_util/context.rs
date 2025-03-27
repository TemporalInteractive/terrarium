use bytemuck::Pod;
use futures::{channel::oneshot, executor::block_on};
use std::{future::IntoFuture, sync::Arc};
use wgpu::{DownlevelCapabilities, Features, Instance, Limits, PowerPreference};
use winit::{
    dpi::PhysicalSize,
    event::{Event, StartCause},
    window::Window,
};

use super::surface::Surface;

pub struct Context {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Context {
    async fn init_with_instance(
        instance: Instance,
        optional_features: Features,
        required_features: Features,
        required_downlevel_capabilities: DownlevelCapabilities,
        required_limits: Limits,
    ) -> Self {
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

        let trace_dir = std::env::var("WGPU_TRACE");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: (optional_features & adapter_features) | required_features,
                    required_limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                trace_dir.ok().as_ref().map(std::path::Path::new),
            )
            .await
            .expect("Unable to find a suitable GPU adapter!");

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }

    pub async fn init_headless(
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

        Self::init_with_instance(
            instance,
            optional_features,
            required_features,
            required_downlevel_capabilities,
            required_limits,
        )
        .await
    }

    pub async fn init_with_window(
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

        Self::init_with_instance(
            instance,
            optional_features,
            required_features,
            required_downlevel_capabilities,
            required_limits,
        )
        .await
    }

    pub fn init_with_xr(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }
}
