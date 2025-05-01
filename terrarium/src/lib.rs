use glam::{UVec2, Vec2, Vec3};
use gpu_resources::{
    sky::{AtmosphereInfo, SunInfo},
    GpuResources,
};
use render_passes::{
    bloom_pass::{self, BloomPassParameters},
    color_correction_pass::{self, ColorCorrectionPassParameters},
    debug_line_pass::{self, DebugLinePassParameters},
    rt_gbuffer_pass::{self, RtGbufferPassParameters},
    shade_pass::{self, ShadePassParameters, ShadingMode},
    shadow_pass::{self, ShadowPassParameters},
    ssao_pass::{self, SsaoPassParameters},
    taa_pass::{self, TaaPassParameters},
};
use world::transform::UP;
use xr::XrCameraState;

pub mod app_loop;
pub mod gpu_resources;
pub mod helpers;
pub mod render_passes;
pub mod wgpu_util;
pub mod world;
pub mod xr;

#[cfg(feature = "egui")]
mod egui_renderer;
#[cfg(feature = "egui")]
pub use egui;

#[repr(C)]
struct PackedGBufferTexel {
    position_ws: Vec3,
    depth_ws: f32,
    normal_ws: u32,
    tangent_ws: u32,
    material_descriptor_idx: u32,
    tex_coord: u32,
    velocity: Vec2,
    ddx: Vec2,
    ddy: Vec2,
    normal_roughness: f32,
    geometric_normal_ws: u32,
}

struct SizedResources {
    resolution: UVec2,
    gbuffer: [wgpu::Buffer; 2],
    shadow_resolution: UVec2,
    shadow_texture: wgpu::Texture,
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
            shadow_texture,
            shadow_texture_view,
        }
    }
}

pub struct RenderSettings {
    pub shading_mode: ShadingMode,
    pub enable_debug_lines: bool,
    pub apply_mipmaps: bool,
    pub apply_normal_maps: bool,
    pub enable_shadows: bool,
    pub enable_ssao: bool,
    pub ssao_intensity: f32,
    pub ssao_sample_count: u32,
    pub enable_bloom: bool,
    pub bloom_intensity: f32,
    pub bloom_radius: f32,
    pub enable_taa: bool,
    pub taa_history_influence: f32,
    pub sun: SunInfo,
    pub atmosphere: AtmosphereInfo,
    pub world_up: Vec3,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            shading_mode: ShadingMode::Full,
            enable_debug_lines: true,
            apply_mipmaps: true,
            apply_normal_maps: true,
            enable_shadows: true,
            enable_ssao: false,
            ssao_intensity: 1.0,
            ssao_sample_count: 8,
            enable_bloom: true,
            bloom_intensity: 0.13,
            bloom_radius: 2.7,
            enable_taa: true,
            taa_history_influence: 0.8,
            sun: SunInfo::default(),
            atmosphere: AtmosphereInfo::default(),
            world_up: UP,
        }
    }
}

impl RenderSettings {
    #[cfg(feature = "egui")]
    pub fn egui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Shading");
        egui::ComboBox::from_label("Visualization Mode")
            .selected_text(format!("{:?}", self.shading_mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.shading_mode, ShadingMode::Full, "Full");
                ui.selectable_value(
                    &mut self.shading_mode,
                    ShadingMode::LightingOnly,
                    "LightingOnly",
                );
                ui.selectable_value(&mut self.shading_mode, ShadingMode::Albedo, "Albedo");
                ui.selectable_value(&mut self.shading_mode, ShadingMode::Normals, "Normals");
                ui.selectable_value(&mut self.shading_mode, ShadingMode::Texcoords, "Texcoords");
            });
        ui.checkbox(&mut self.enable_debug_lines, "Debug Lines");
        ui.checkbox(&mut self.apply_mipmaps, "Mipmapping");
        ui.checkbox(&mut self.apply_normal_maps, "Normal Mapping");
        ui.separator();

        ui.heading("Shadows");
        ui.checkbox(&mut self.enable_shadows, "Enable");
        ui.separator();

        ui.heading("Sun");
        self.sun.egui(ui);
        ui.separator();
        ui.heading("Atmosphere");
        self.atmosphere.egui(ui);
        ui.separator();

        ui.heading("Ssao");
        ui.checkbox(&mut self.enable_ssao, "Enable");
        ui.add(egui::Slider::new(&mut self.ssao_intensity, 0.0..=1.0).text("Intensity"));
        egui::ComboBox::from_label("Sample Count")
            .selected_text(format!("{:?}", self.ssao_sample_count))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.ssao_sample_count, 8, "8");
                ui.selectable_value(&mut self.ssao_sample_count, 16, "16");
                ui.selectable_value(&mut self.ssao_sample_count, 32, "32");
            });
        ui.separator();

        ui.heading("Bloom");
        ui.checkbox(&mut self.enable_bloom, "Enable");
        ui.add(egui::Slider::new(&mut self.bloom_intensity, 0.0..=1.0).text("Intensity"));
        ui.add(egui::Slider::new(&mut self.bloom_radius, 0.0..=10.0).text("Radius"));
        ui.separator();

        ui.heading("Taa");
        ui.checkbox(&mut self.enable_taa, "Enable");
        ui.add(
            egui::Slider::new(&mut self.taa_history_influence, 0.0..=1.0).text("History Influence"),
        );
    }
}

pub struct RenderParameters<'a> {
    pub render_settings: &'a RenderSettings,
    pub xr_camera_state: &'a XrCameraState,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub render_target: &'a wgpu::Texture,
    pub prev_render_target: &'a wgpu::Texture,
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
        parameters.gpu_resources.sky_mut().constants.sun = parameters.render_settings.sun;
        parameters.gpu_resources.sky_mut().constants.atmosphere =
            parameters.render_settings.atmosphere;
        parameters.gpu_resources.sky_mut().constants.world_up = parameters.render_settings.world_up;

        parameters.gpu_resources.update(
            parameters.xr_camera_state,
            parameters.world,
            command_encoder,
            &ctx.queue,
        );

        rt_gbuffer_pass::encode(
            &RtGbufferPassParameters {
                resolution: self.sized_resources.resolution,
                mipmapping: parameters.render_settings.apply_mipmaps,
                normal_mapping: parameters.render_settings.apply_normal_maps,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                gbuffer: &self.sized_resources.gbuffer,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        if parameters.render_settings.enable_shadows {
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
        } else {
            command_encoder.clear_texture(
                &self.sized_resources.shadow_texture,
                &wgpu::ImageSubresourceRange::default(),
            );
        }

        if parameters.render_settings.enable_ssao {
            ssao_pass::encode(
                &SsaoPassParameters {
                    resolution: self.sized_resources.resolution,
                    shadow_resolution: self.sized_resources.shadow_resolution,
                    seed: self.frame_idx,
                    sample_count: parameters.render_settings.ssao_sample_count,
                    radius: 1.0,
                    intensity: parameters.render_settings.ssao_intensity,
                    bias: 0.01,
                    shadow_texture_view: &self.sized_resources.shadow_texture_view,
                    xr_camera_buffer: parameters.xr_camera_buffer,
                    gbuffer: &self.sized_resources.gbuffer,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        }

        let rt_view = parameters
            .render_target
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                mip_level_count: Some(1),
                ..Default::default()
            });
        let prev_rt_view =
            parameters
                .prev_render_target
                .create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    array_layer_count: Some(2),
                    mip_level_count: Some(1),
                    ..Default::default()
                });

        shade_pass::encode(
            &ShadePassParameters {
                resolution: self.sized_resources.resolution,
                shading_mode: parameters.render_settings.shading_mode,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                gbuffer: &self.sized_resources.gbuffer,
                shadow_texture_view: &self.sized_resources.shadow_texture_view,
                dst_view: &rt_view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        if parameters.render_settings.enable_bloom {
            bloom_pass::encode(
                &BloomPassParameters {
                    intensity: parameters.render_settings.bloom_intensity,
                    radius: parameters.render_settings.bloom_radius,
                    color_texture: parameters.render_target,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        }

        if parameters.render_settings.enable_debug_lines {
            debug_line_pass::encode(
                &DebugLinePassParameters {
                    gpu_resources: parameters.gpu_resources,
                    xr_camera_buffer: parameters.xr_camera_buffer,
                    dst_view: &rt_view,
                    target_format: wgpu::TextureFormat::Rgba16Float,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        }

        color_correction_pass::encode(
            &ColorCorrectionPassParameters {
                resolution: self.sized_resources.resolution,
                color_texture_view: &rt_view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        if parameters.render_settings.enable_taa {
            taa_pass::encode(
                &TaaPassParameters {
                    resolution: self.sized_resources.resolution,
                    history_influence: parameters.render_settings.taa_history_influence,
                    color_texture_view: &rt_view,
                    prev_color_texture_view: &prev_rt_view,
                    gbuffer: &self.sized_resources.gbuffer,
                    xr_camera_buffer: parameters.xr_camera_buffer,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        }

        parameters.gpu_resources.end_frame();
        self.frame_idx += 1;
    }

    pub fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &wgpu_util::Context) {
        self.sized_resources =
            SizedResources::new(config, self.shadow_resolution_scale, &ctx.device);
    }

    pub fn required_features() -> wgpu::Features {
        wgpu::Features::MULTIVIEW
            | wgpu::Features::PUSH_CONSTANTS
            | wgpu::Features::TEXTURE_BINDING_ARRAY
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
            | wgpu::Features::EXPERIMENTAL_RAY_TRACING_ACCELERATION_STRUCTURE
            | wgpu::Features::EXPERIMENTAL_RAY_QUERY
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::FLOAT32_FILTERABLE
            | wgpu::Features::CLEAR_TEXTURE
            | wgpu::Features::POLYGON_MODE_LINE
    }
}
