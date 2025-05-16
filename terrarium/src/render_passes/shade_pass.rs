use std::fmt;

use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::{gbuffer::Gbuffer, GpuResources},
    wgpu_util::{ComputePipelineDescriptorExtensions, PipelineDatabase},
};

use super::build_frustum_pass;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ShadingMode {
    #[default]
    Full,
    LightingOnly,
    Albedo,
    Normals,
    Texcoords,
    Emission,
    Velocity,
    Fog,
    SimpleLighting,
}

impl fmt::Display for ShadingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Full => "Full",
            Self::LightingOnly => "Lighting Only",
            Self::Albedo => "Albedo",
            Self::Normals => "Normals",
            Self::Texcoords => "Texcoords",
            Self::Emission => "Emission",
            Self::Velocity => "Velocity",
            Self::Fog => "Fog",
            Self::SimpleLighting => "Simple Lighting",
        };
        write!(f, "{}", name)
    }
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    shading_mode: u32,
    ambient_factor: f32,
}

pub struct ShadePassParameters<'a> {
    pub resolution: UVec2,
    pub shading_mode: ShadingMode,
    pub ambient_factor: f32,
    pub gpu_resources: &'a GpuResources,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub gbuffer: &'a Gbuffer,
    pub lighting_view: &'a wgpu::TextureView,
    pub dst_view: &'a wgpu::TextureView,
}

pub fn encode(
    parameters: &ShadePassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader =
        pipeline_database.shader_from_src(device, include_wgsl!("../../shaders/shade_pass.wgsl"));
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::shade"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::shade"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 4,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::Rgba16Float,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 5,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 6,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    }),
                    parameters.gpu_resources.vertex_pool().bind_group_layout(),
                    parameters.gpu_resources.material_pool().bind_group_layout(),
                    parameters.gpu_resources.sky().bind_group_layout(),
                    parameters.gbuffer.bind_group_layout(),
                    parameters
                        .gpu_resources
                        .linear_transformed_cosines()
                        .bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::shade constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            shading_mode: parameters.shading_mode as u32,
            ambient_factor: parameters.ambient_factor,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let lighting_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        min_filter: wgpu::FilterMode::Linear,
        mag_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: constants.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: parameters.xr_camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(parameters.dst_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(parameters.lighting_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(&lighting_sampler),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::shade"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_bind_group(
            1,
            &parameters.gpu_resources.vertex_pool().bind_group(device),
            &[],
        );
        parameters.gpu_resources.material_pool().bind_group(
            pipeline.get_bind_group_layout(2),
            device,
            |bind_group| {
                cpass.set_bind_group(2, bind_group, &[]);
            },
        );
        cpass.set_bind_group(3, &parameters.gpu_resources.sky().bind_group(device), &[]);
        cpass.set_bind_group(4, parameters.gbuffer.bind_group(), &[]);
        cpass.set_bind_group(
            5,
            parameters
                .gpu_resources
                .linear_transformed_cosines()
                .bind_group(),
            &[],
        );
        cpass.insert_debug_marker("terrarium::shade");
        cpass.dispatch_workgroups(
            parameters
                .resolution
                .x
                .div_ceil(build_frustum_pass::TILE_SIZE),
            parameters
                .resolution
                .y
                .div_ceil(build_frustum_pass::TILE_SIZE),
            1,
        );
    }
}
