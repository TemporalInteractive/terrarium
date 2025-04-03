use glam::UVec2;
use gpu_resources::GpuResources;
use render_passes::{
    rt_gbuffer_pass::{self, RtGbufferPassParameters},
    shade_pass::{self, ShadePassParameters},
    shadow_pass::{self, ShadowPassParameters},
    taa_pass::{self, TaaPassParameters},
};

pub mod app_loop;
pub mod gpu_resources;
pub mod helpers;
pub mod render_passes;
pub mod wgpu_util;
pub mod world;
pub mod xr;

#[repr(C)]
struct PackedGBufferTexel {
    depth_ws: f32,
    normal_ws: u32,
    material_descriptor_idx: u32,
    tex_coord: u32,
}

struct SizedResources {
    resolution: UVec2,
    gbuffer: [wgpu::Buffer; 2],
    shadow_resolution: UVec2,
    shadow_texture_view: wgpu::TextureView,
}

impl SizedResources {
    pub fn new(
        config: &wgpu::SurfaceConfiguration,
        shadow_resolution_scale: f32,
        device: &wgpu::Device,
    ) -> Self {
        let resolution = UVec2::new(config.width, config.height);

        let gbuffer = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("terrarium::gbuffer {}", i)),
                size: size_of::<PackedGBufferTexel>() as u64 * (resolution.x * resolution.y) as u64,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            })
        });

        let shadow_resolution = UVec2::new(
            (config.width as f32 * shadow_resolution_scale).ceil() as u32,
            (config.height as f32 * shadow_resolution_scale).ceil() as u32,
        );

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrarium::shadow"),
            size: wgpu::Extent3d {
                width: shadow_resolution.x,
                height: shadow_resolution.y,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let shadow_texture_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            ..Default::default()
        });

        Self {
            resolution,
            gbuffer,
            shadow_resolution,
            shadow_texture_view,
        }
    }
}

pub struct RenderParameters<'a> {
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub view: &'a wgpu::TextureView,
    pub prev_view: &'a wgpu::TextureView,
    pub world: &'a specs::World,
    pub gpu_resources: &'a mut GpuResources,
}

pub struct Renderer {
    sized_resources: SizedResources,
    shadow_resolution_scale: f32,
    frame_idx: u32,
}

impl Renderer {
    pub fn new(config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) -> Self {
        let shadow_resolution_scale = 1.0;

        let sized_resources = SizedResources::new(config, shadow_resolution_scale, &ctx.device);

        Self {
            sized_resources,
            shadow_resolution_scale,
            frame_idx: 0,
        }
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

        rt_gbuffer_pass::encode(
            &RtGbufferPassParameters {
                resolution: self.sized_resources.resolution,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                gbuffer: &self.sized_resources.gbuffer,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        shadow_pass::encode(
            &ShadowPassParameters {
                resolution: self.sized_resources.resolution,
                shadow_resolution: self.sized_resources.shadow_resolution,
                seed: self.frame_idx,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                gbuffer: &self.sized_resources.gbuffer,
                shadow_texture_view: &self.sized_resources.shadow_texture_view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        shade_pass::encode(
            &ShadePassParameters {
                resolution: self.sized_resources.resolution,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                gbuffer: &self.sized_resources.gbuffer,
                shadow_texture_view: &self.sized_resources.shadow_texture_view,
                dst_view: parameters.view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        // taa_pass::encode(
        //     &TaaPassParameters {
        //         resolution: self.sized_resources.resolution,
        //         history_influence: 0.8,
        //         color_texture_view: parameters.view,
        //         prev_color_texture_view: parameters.prev_view,
        //     },
        //     &ctx.device,
        //     command_encoder,
        //     pipeline_database,
        // );

        parameters.gpu_resources.end_frame();
        self.frame_idx += 1;
    }

    pub fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) {
        self.sized_resources =
            SizedResources::new(config, self.shadow_resolution_scale, &ctx.device);
    }
}
