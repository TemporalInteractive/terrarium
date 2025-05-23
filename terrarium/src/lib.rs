use std::num::NonZeroU32;

use glam::{UVec2, Vec3};
use gpu_resources::{
    gbuffer::Gbuffer,
    sky::{AtmosphereInfo, SunInfo},
    GpuResources,
};
use render_passes::{
    blit_pass::{self, BlitPassParameters},
    bloom_pass::{self, BloomPassParameters},
    build_frustum_pass::{self, BuildFrustumPassParameters},
    color_correction_pass::{self, ColorCorrectionPassParameters},
    debug_line_pass::{self, DebugLinePassParameters},
    ltc_cull_pass::{self, LtcCullPassParameters},
    ltc_lighting_pass::{self, LtcLightingPassParameters},
    mirror_reflection_pass::{self, MirrorReflectionPassParameters},
    rt_gbuffer_pass::{self, RtGbufferPassParameters},
    shade_pass::{self, ShadePassParameters, ShadingMode},
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

struct SizedResources {
    resolution: UVec2,
    render_resolution: UVec2,
    lighting_resolution: UVec2,
    render_resolution_scale: f32,
    lighting_resolution_scale: f32,

    reflection_counter_buffer: [wgpu::Buffer; 2],
    reflection_pid_buffer: [wgpu::Buffer; 2],
    frustum_buffer: wgpu::Buffer,
    ltc_instance_index_buffer: wgpu::Buffer,
    ltc_instance_grid_texture_view: wgpu::TextureView,
    gbuffer: Gbuffer,
    shading_texture: [wgpu::Texture; 2],
    lighting_texture: wgpu::Texture,
    reflection_texture: wgpu::Texture,
}

impl SizedResources {
    pub fn new(
        resolution: UVec2,
        render_resolution_scale: f32,
        lighting_resolution_scale: f32,
        device: &wgpu::Device,
    ) -> Self {
        let render_resolution = UVec2::new(
            (resolution.x as f32 * render_resolution_scale).ceil() as u32,
            (resolution.y as f32 * render_resolution_scale).ceil() as u32,
        );
        let lighting_resolution = UVec2::new(
            (render_resolution.x as f32 * lighting_resolution_scale).ceil() as u32,
            (render_resolution.y as f32 * lighting_resolution_scale).ceil() as u32,
        );

        let gbuffer = Gbuffer::new(render_resolution, device);

        let shading_texture = std::array::from_fn(|i| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("terrarium::shading {}", i)),
                size: wgpu::Extent3d {
                    width: render_resolution.x,
                    height: render_resolution.y,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
        });

        let lighting_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrarium::lighting"),
            size: wgpu::Extent3d {
                width: lighting_resolution.x,
                height: lighting_resolution.y,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let reflection_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrarium::reflection"),
            size: wgpu::Extent3d {
                width: render_resolution.x,
                height: render_resolution.y,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let reflection_counter_buffer = std::array::from_fn(|_| {
            rt_gbuffer_pass::create_reflection_counter_buffer(resolution, device)
        });
        let reflection_pid_buffer = std::array::from_fn(|_| {
            rt_gbuffer_pass::create_reflection_pid_buffer(resolution, device)
        });

        let frustum_buffer = build_frustum_pass::create_frustum_buffer(lighting_resolution, device);

        let ltc_instance_index_buffer =
            ltc_cull_pass::create_ltc_instance_index_buffer(lighting_resolution, device);
        let ltc_instance_grid_texture_view =
            ltc_cull_pass::create_ltc_instance_grid_texture(lighting_resolution, device);

        device.poll(wgpu::PollType::Wait).unwrap();

        Self {
            resolution,
            render_resolution,
            lighting_resolution,
            render_resolution_scale,
            lighting_resolution_scale,

            reflection_counter_buffer,
            reflection_pid_buffer,
            frustum_buffer,
            ltc_instance_index_buffer,
            ltc_instance_grid_texture_view,
            gbuffer,
            shading_texture,
            lighting_texture,
            reflection_texture,
        }
    }
}

pub struct RenderSettings {
    pub render_resolution_scale: f32,
    pub shading_mode: ShadingMode,
    pub render_distance: f32,
    pub ambient_factor: f32,
    pub enable_lighting: bool,
    pub enable_shadows: bool,
    pub shadow_bias: f32,
    pub lighting_range_bias: f32,
    pub lighting_resolution_scale: f32,
    pub enable_reflections: bool,
    pub reflection_max_roughness: f32,
    pub enable_debug_lines: bool,
    pub apply_mipmaps: bool,
    pub apply_normal_maps: bool,
    pub enable_bloom: bool,
    pub bloom_intensity: f32,
    pub bloom_radius: f32,
    pub enable_emissive_stabilization: bool,
    pub enable_taa: bool,
    pub sun: SunInfo,
    pub atmosphere: AtmosphereInfo,
    pub world_up: Vec3,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            render_resolution_scale: 1.0,
            shading_mode: ShadingMode::Full,
            render_distance: 1000.0,
            ambient_factor: 0.1,
            enable_lighting: true,
            enable_shadows: true,
            shadow_bias: 0.1,
            lighting_range_bias: 0.0,
            lighting_resolution_scale: 0.9,
            enable_reflections: true,
            reflection_max_roughness: 0.2,
            enable_debug_lines: true,
            apply_mipmaps: true,
            apply_normal_maps: true,
            enable_bloom: true,
            bloom_intensity: 0.04,
            bloom_radius: 1.0,
            enable_emissive_stabilization: true,
            enable_taa: true,
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
        ui.add(
            egui::Slider::new(&mut self.render_resolution_scale, 0.4..=1.0)
                .text("Resolution Scale"),
        );
        egui::ComboBox::from_label("Visualization Mode")
            .selected_text(self.shading_mode.to_string())
            .show_ui(ui, |ui| {
                for mode in [
                    ShadingMode::Full,
                    ShadingMode::LightingOnly,
                    ShadingMode::Albedo,
                    ShadingMode::Normals,
                    ShadingMode::Texcoords,
                    ShadingMode::Emission,
                    ShadingMode::Velocity,
                    ShadingMode::Fog,
                    ShadingMode::Reflection,
                    ShadingMode::SimpleLighting,
                ] {
                    ui.selectable_value(&mut self.shading_mode, mode, mode.to_string());
                }
            });
        ui.add(egui::Slider::new(&mut self.render_distance, 0.0..=10000.0).text("Render Distance"));
        ui.add(egui::Slider::new(&mut self.ambient_factor, 0.0..=1.0).text("Ambient Factor"));
        ui.checkbox(&mut self.enable_debug_lines, "Debug Lines");
        ui.checkbox(&mut self.apply_mipmaps, "Mipmapping");
        ui.checkbox(&mut self.apply_normal_maps, "Normal Mapping");
        ui.separator();

        ui.heading("Lighting");
        ui.checkbox(&mut self.enable_lighting, "Enable");
        ui.add(
            egui::Slider::new(&mut self.lighting_resolution_scale, 0.4..=1.0)
                .text("Resolution Scale"),
        );
        ui.add(egui::Slider::new(&mut self.lighting_range_bias, 0.0..=0.3).text("Range Bias"));
        ui.checkbox(&mut self.enable_shadows, "Shadows");
        ui.add(egui::Slider::new(&mut self.shadow_bias, 0.0..=1.0).text("Shadow Bias"));
        ui.separator();

        ui.heading("Reflections");
        ui.checkbox(&mut self.enable_reflections, "Enable");
        ui.add(
            egui::Slider::new(&mut self.reflection_max_roughness, 0.0..=1.0).text("Max Roughness"),
        );
        ui.separator();

        ui.heading("Sun");
        self.sun.egui(ui);
        ui.separator();
        ui.heading("Atmosphere");
        self.atmosphere.egui(ui);
        ui.separator();

        ui.heading("Bloom");
        ui.checkbox(&mut self.enable_bloom, "Enable");
        ui.add(egui::Slider::new(&mut self.bloom_intensity, 0.0..=1.0).text("Intensity"));
        ui.add(egui::Slider::new(&mut self.bloom_radius, 0.0..=10.0).text("Radius"));
        ui.separator();

        ui.heading("Emissive Stabilisation");
        ui.checkbox(&mut self.enable_emissive_stabilization, "Enable");

        ui.heading("Taa");
        ui.checkbox(&mut self.enable_taa, "Enable");
    }
}

pub struct RenderParameters<'a> {
    pub render_settings: &'a RenderSettings,
    pub xr_camera_state: &'a XrCameraState,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub render_target: &'a wgpu::Texture,
    pub world: &'a specs::World,
    pub gpu_resources: &'a mut GpuResources,
    #[cfg(feature = "transform-gizmo")]
    pub gizmo_draw_data: Option<transform_gizmo::GizmoDrawData>,
}

pub struct Renderer {
    sized_resources: SizedResources,
    frame_idx: u32,
}

impl Renderer {
    pub fn new(resolution: UVec2, ctx: &wgpu_util::Context) -> Self {
        let sized_resources = SizedResources::new(resolution, 1.0, 1.0, &ctx.device);

        Self {
            sized_resources,
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
        if parameters.render_settings.render_resolution_scale
            != self.sized_resources.render_resolution_scale
            || parameters.render_settings.lighting_resolution_scale
                != self.sized_resources.lighting_resolution_scale
        {
            self.sized_resources = SizedResources::new(
                self.sized_resources.resolution,
                parameters.render_settings.render_resolution_scale,
                parameters.render_settings.lighting_resolution_scale,
                &ctx.device,
            );
        }

        parameters.gpu_resources.sky_mut().constants.sun = parameters.render_settings.sun;
        parameters.gpu_resources.sky_mut().constants.atmosphere =
            parameters.render_settings.atmosphere;
        parameters.gpu_resources.sky_mut().constants.world_up = parameters.render_settings.world_up;

        parameters
            .gpu_resources
            .linear_transformed_cosines_mut()
            .range_bias = parameters.render_settings.lighting_range_bias;

        parameters.gpu_resources.update(
            parameters.world,
            parameters.xr_camera_state,
            command_encoder,
            &ctx.queue,
        );

        rt_gbuffer_pass::encode(
            &RtGbufferPassParameters {
                resolution: self.sized_resources.render_resolution,
                mipmapping: parameters.render_settings.apply_mipmaps,
                normal_mapping: parameters.render_settings.apply_normal_maps,
                reflection_max_roughness: parameters.render_settings.reflection_max_roughness,
                render_distance: parameters.render_settings.render_distance,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                gbuffer: &self.sized_resources.gbuffer,
                reflection_counter_buffer: &self.sized_resources.reflection_counter_buffer,
                reflection_pid_buffer: &self.sized_resources.reflection_pid_buffer,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        build_frustum_pass::encode(
            &BuildFrustumPassParameters {
                resolution: self.sized_resources.render_resolution,
                lighting_resolution: self.sized_resources.lighting_resolution,
                gbuffer: &self.sized_resources.gbuffer,
                xr_camera_buffer: parameters.xr_camera_buffer,
                frustum_buffer: &self.sized_resources.frustum_buffer,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        ltc_cull_pass::encode(
            &LtcCullPassParameters {
                resolution: self.sized_resources.lighting_resolution,
                gpu_resources: parameters.gpu_resources,
                frustum_buffer: &self.sized_resources.frustum_buffer,
                ltc_instance_index_buffer: &self.sized_resources.ltc_instance_index_buffer,
                ltc_instance_grid_texture_view: &self
                    .sized_resources
                    .ltc_instance_grid_texture_view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        let lighting_view =
            self.sized_resources
                .lighting_texture
                .create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    ..Default::default()
                });

        if parameters.render_settings.enable_lighting {
            ltc_lighting_pass::encode(
                &LtcLightingPassParameters {
                    resolution: self.sized_resources.render_resolution,
                    lighting_resolution: self.sized_resources.lighting_resolution,
                    shadows: parameters.render_settings.enable_shadows,
                    shadow_bias: parameters.render_settings.shadow_bias,
                    gpu_resources: parameters.gpu_resources,
                    xr_camera_buffer: parameters.xr_camera_buffer,
                    gbuffer: &self.sized_resources.gbuffer,
                    ltc_instance_index_buffer: &self.sized_resources.ltc_instance_index_buffer,
                    ltc_instance_grid_texture_view: &self
                        .sized_resources
                        .ltc_instance_grid_texture_view,
                    dst_view: &lighting_view,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        } else {
            command_encoder.clear_texture(
                &self.sized_resources.lighting_texture,
                &wgpu::ImageSubresourceRange::default(),
            );
        }

        let reflection_view =
            self.sized_resources
                .reflection_texture
                .create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    ..Default::default()
                });

        if parameters.render_settings.enable_reflections {
            mirror_reflection_pass::encode(
                &MirrorReflectionPassParameters {
                    resolution: self.sized_resources.render_resolution,
                    reflection_resolution: self.sized_resources.render_resolution, // TODO!
                    ambient_factor: parameters.render_settings.ambient_factor,
                    render_distance: parameters.render_settings.render_distance,
                    gpu_resources: parameters.gpu_resources,
                    xr_camera_buffer: parameters.xr_camera_buffer,
                    gbuffer: &self.sized_resources.gbuffer,
                    reflection_counter_buffer: &self.sized_resources.reflection_counter_buffer,
                    reflection_pid_buffer: &self.sized_resources.reflection_pid_buffer,
                    dst_view: &reflection_view,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        } else {
            command_encoder.clear_texture(
                &self.sized_resources.reflection_texture,
                &wgpu::ImageSubresourceRange::default(),
            );
        }

        let shading_view = self.sized_resources.shading_texture[self.frame_idx as usize % 2]
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                mip_level_count: Some(1),
                ..Default::default()
            });
        let prev_shading_view = self.sized_resources.shading_texture
            [(self.frame_idx as usize + 1) % 2]
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                mip_level_count: Some(1),
                ..Default::default()
            });

        shade_pass::encode(
            &ShadePassParameters {
                resolution: self.sized_resources.render_resolution,
                shading_mode: parameters.render_settings.shading_mode,
                ambient_factor: parameters.render_settings.ambient_factor,
                reflection_max_roughness: parameters.render_settings.reflection_max_roughness,
                gpu_resources: parameters.gpu_resources,
                xr_camera_buffer: parameters.xr_camera_buffer,
                gbuffer: &self.sized_resources.gbuffer,
                lighting_view: &lighting_view,
                reflection_view: &reflection_view,
                dst_view: &shading_view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        if parameters.render_settings.enable_taa {
            taa_pass::encode(
                &TaaPassParameters {
                    resolution: self.sized_resources.render_resolution,
                    color_texture_view: &shading_view,
                    prev_color_texture_view: &prev_shading_view,
                    gbuffer: &self.sized_resources.gbuffer,
                    xr_camera_buffer: parameters.xr_camera_buffer,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        }

        let render_target_view =
            parameters
                .render_target
                .create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    array_layer_count: Some(2),
                    mip_level_count: Some(1),
                    ..Default::default()
                });

        blit_pass::encode(
            &BlitPassParameters {
                src_view: &shading_view,
                dst_view: &render_target_view,
                multiview: Some(NonZeroU32::new(2).unwrap()),
                view_index_override: None,
                target_format: wgpu::TextureFormat::Rgba16Float,
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
                    initial_color_texture: &self.sized_resources.shading_texture
                        [self.frame_idx as usize % 2],
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
                    dst_view: &render_target_view,
                    target_format: wgpu::TextureFormat::Rgba16Float,
                },
                &ctx.device,
                command_encoder,
                pipeline_database,
            );
        }

        #[cfg(feature = "transform-gizmo")]
        if let Some(gizmo_draw_data) = &parameters.gizmo_draw_data {
            use crate::render_passes::gizmo_pass::{self, GizmoPassParameters};

            gizmo_pass::encode(
                &GizmoPassParameters {
                    resolution: self.sized_resources.resolution,
                    gizmo_draw_data,
                    dst_view: &render_target_view,
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
                color_texture_view: &render_target_view,
            },
            &ctx.device,
            command_encoder,
            pipeline_database,
        );

        parameters.gpu_resources.end_frame(command_encoder);
        self.frame_idx += 1;
    }

    pub fn resize(&mut self, resolution: UVec2, ctx: &wgpu_util::Context) {
        self.sized_resources = SizedResources::new(
            resolution,
            self.sized_resources.render_resolution_scale,
            self.sized_resources.lighting_resolution_scale,
            &ctx.device,
        );
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
