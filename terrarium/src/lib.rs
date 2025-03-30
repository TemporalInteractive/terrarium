use glam::UVec2;
use gpu_resources::GpuResources;
use render_passes::rt_gbuffer_pass::{self, RtGbufferPassParameters};

use crate::render_passes::gbuffer_pass::{self, GbufferPassParameters};

pub mod app_loop;
pub mod gpu_resources;
pub mod helpers;
pub mod render_passes;
pub mod wgpu_util;
pub mod world;
pub mod xr;

struct SizedResources {
    resolution: UVec2,
    depth_texture: wgpu::Texture,
}

impl SizedResources {
    pub fn new(config: &wgpu::SurfaceConfiguration, device: &wgpu::Device) -> Self {
        let resolution = UVec2::new(config.width, config.height);

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrarium::depth"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        Self {
            resolution,
            depth_texture,
        }
    }
}

pub struct RenderParameters<'a> {
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub view: &'a wgpu::TextureView,
    pub world: &'a specs::World,
    pub gpu_resources: &'a mut GpuResources,
}

pub struct Renderer {
    sized_resources: SizedResources,
}

impl Renderer {
    pub fn new(config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) -> Self {
        let sized_resources = SizedResources::new(config, &ctx.device);

        Self { sized_resources }
    }

    pub fn render(
        &mut self,
        parameters: &mut RenderParameters,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
        pipeline_database: &mut wgpu_util::PipelineDatabase,
    ) {
        parameters
            .gpu_resources
            .update(parameters.world, command_encoder, &ctx.queue);

        // gbuffer_pass::encode(
        //     &GbufferPassParameters {
        //         world: parameters.world,
        //         gpu_resources: parameters.gpu_resources,
        //         xr_camera_buffer: parameters.xr_camera_buffer,
        //         dst_view: parameters.view,
        //         target_format: wgpu::TextureFormat::Rgba8Unorm,
        //         depth_texture: &self.sized_resources.depth_texture,
        //     },
        //     &ctx.device,
        //     command_encoder,
        //     pipeline_database,
        // );

        rt_gbuffer_pass::encode(
            &RtGbufferPassParameters {
                resolution: self.sized_resources.resolution,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                dst_view: parameters.view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        parameters.gpu_resources.end_frame();
    }

    pub fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) {
        self.sized_resources = SizedResources::new(config, &ctx.device);
    }
}
