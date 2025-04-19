use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::wgpu_util::{ComputePipelineDescriptorExtensions, PipelineDatabase};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    src_resolution: UVec2,
    dst_resolution: UVec2,
    radius: f32,
    intensity: f32,
    src_mip_level: u32,
    _padding0: u32,
}

pub struct BloomPassParameters<'a> {
    pub intensity: f32,
    pub radius: f32,
    pub color_texture: &'a wgpu::Texture,
}

pub fn encode(
    parameters: &BloomPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let mip_level_count = parameters.color_texture.mip_level_count();

    let mut src_resolution = UVec2::new(
        parameters.color_texture.width(),
        parameters.color_texture.height(),
    );

    let mut mip_resolutions = vec![src_resolution];

    for i in 0..mip_level_count - 1 {
        let dst_resolution = (src_resolution / 2).max(UVec2::ONE);

        encode_downsample(
            parameters,
            src_resolution,
            dst_resolution,
            i,
            &sampler,
            device,
            command_encoder,
            pipeline_database,
        );

        src_resolution = dst_resolution;
        mip_resolutions.push(src_resolution);
    }

    for i in (1..mip_level_count).rev() {
        // println!("{} {}", mip_level_count, i);
        // let dst_resolution = (src_resolution * 2).min(UVec2::new(
        //     parameters.color_texture.width(),
        //     parameters.color_texture.height(),
        // ));
        let src_resolution = mip_resolutions[i as usize];
        let dst_resolution = mip_resolutions[i as usize - 1];

        encode_upsample(
            parameters,
            src_resolution,
            dst_resolution,
            i,
            &sampler,
            device,
            command_encoder,
            pipeline_database,
        );

        //src_resolution = dst_resolution;
    }
}

fn encode_downsample(
    parameters: &BloomPassParameters,
    src_resolution: UVec2,
    dst_resolution: UVec2,
    src_mip_level: u32,
    sampler: &wgpu::Sampler,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_wgsl!("../../shaders/bloom_downsample_pass.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::bloom_downsample"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::bloom_downsample"),
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
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
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::Rgba32Float,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                },
                                count: None,
                            },
                        ],
                    },
                )],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::bloom_downsample constants"),
        contents: bytemuck::bytes_of(&Constants {
            src_resolution,
            dst_resolution,
            radius: parameters.radius,
            intensity: parameters.intensity,
            src_mip_level,
            _padding0: 0,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let src_view = parameters
        .color_texture
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            mip_level_count: Some(1),
            base_mip_level: src_mip_level,
            ..Default::default()
        });
    let dst_view = parameters
        .color_texture
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            mip_level_count: Some(1),
            base_mip_level: src_mip_level + 1,
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
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&dst_view),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::bloom_downsample"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("terrarium::bloom_downsample");
        cpass.dispatch_workgroups(
            dst_resolution.x.div_ceil(16),
            dst_resolution.y.div_ceil(16),
            1,
        );
    }
}

fn encode_upsample(
    parameters: &BloomPassParameters,
    src_resolution: UVec2,
    dst_resolution: UVec2,
    src_mip_level: u32,
    sampler: &wgpu::Sampler,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_wgsl!("../../shaders/bloom_upsample_pass.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::bloom_upsample"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::bloom_upsample"),
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
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
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::Rgba32Float,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                },
                                count: None,
                            },
                        ],
                    },
                )],
                push_constant_ranges: &[],
            })
        },
    );

    let intensity = if src_mip_level == 1 {
        parameters.intensity
    } else {
        1.0
    };

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::bloom_upsample constants"),
        contents: bytemuck::bytes_of(&Constants {
            src_resolution,
            dst_resolution,
            radius: parameters.radius,
            intensity,
            src_mip_level,
            _padding0: 0,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let src_view = parameters
        .color_texture
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            mip_level_count: Some(1),
            base_mip_level: src_mip_level,
            ..Default::default()
        });
    let dst_view = parameters
        .color_texture
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(2),
            mip_level_count: Some(1),
            base_mip_level: src_mip_level - 1,
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
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&dst_view),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::bloom_upsample"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("terrarium::bloom_upsample");
        cpass.dispatch_workgroups(
            dst_resolution.x.div_ceil(16),
            dst_resolution.y.div_ceil(16),
            1,
        );
    }
}
