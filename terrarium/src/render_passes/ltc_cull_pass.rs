use bytemuck::{Pod, Zeroable};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::GpuResources,
    wgpu_util::{
        empty_bind_group, empty_bind_group_layout, ComputePipelineDescriptorExtensions,
        PipelineDatabase,
    },
};

use super::build_frustum_pass;

const MAX_LTC_INSTANCES_PER_TILE: usize = 128;

pub fn create_ltc_instance_index_buffer(resolution: UVec2, device: &wgpu::Device) -> wgpu::Buffer {
    let num_groups = (resolution.x.div_ceil(build_frustum_pass::TILE_SIZE)
        * resolution.y.div_ceil(build_frustum_pass::TILE_SIZE)) as usize;

    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrarium::ltc_cull_pass ltc_instance_indices"),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        size: (size_of::<u32>() * MAX_LTC_INSTANCES_PER_TILE * num_groups) as u64,
        mapped_at_creation: false,
    })
}

pub fn create_ltc_instance_grid_texture(
    resolution: UVec2,
    device: &wgpu::Device,
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrarium::ltc_cull_pass ltc_instance_grid"),
        size: wgpu::Extent3d {
            width: resolution.x.div_ceil(build_frustum_pass::TILE_SIZE),
            height: resolution.y.div_ceil(build_frustum_pass::TILE_SIZE),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });

    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        ..Default::default()
    })
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    tile_resolution: UVec2,
}

pub struct LtcCullPassParameters<'a> {
    pub resolution: UVec2,
    pub gpu_resources: &'a GpuResources,
    pub frustum_buffer: &'a wgpu::Buffer,
    pub ltc_instance_index_buffer: &'a wgpu::Buffer,
    pub ltc_instance_grid_texture_view: &'a wgpu::TextureView,
}

pub fn encode(
    parameters: &LtcCullPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database
        .shader_from_src(device, include_wgsl!("../../shaders/ltc_cull_pass.wgsl"));
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::ltc_cull"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::ltc_cull"),
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
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
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
                                    format: wgpu::TextureFormat::Rg32Uint,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                },
                                count: None,
                            },
                        ],
                    }),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    parameters
                        .gpu_resources
                        .linear_transformed_cosines()
                        .bind_group_layout(),
                    parameters.gpu_resources.debug_lines().bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let tile_resolution = UVec2::new(
        parameters
            .resolution
            .x
            .div_ceil(build_frustum_pass::TILE_SIZE),
        parameters
            .resolution
            .y
            .div_ceil(build_frustum_pass::TILE_SIZE),
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::ltc_cull constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            tile_resolution,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let light_index_counter = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::ltc_cull light_index_counter"),
        contents: bytemuck::bytes_of(&0u32),
        usage: wgpu::BufferUsages::STORAGE,
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
                resource: parameters.frustum_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: light_index_counter.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: parameters.ltc_instance_index_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(
                    parameters.ltc_instance_grid_texture_view,
                ),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::ltc_cull"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_bind_group(1, empty_bind_group(device), &[]);
        cpass.set_bind_group(2, empty_bind_group(device), &[]);
        cpass.set_bind_group(3, empty_bind_group(device), &[]);
        cpass.set_bind_group(4, empty_bind_group(device), &[]);
        cpass.set_bind_group(
            5,
            parameters
                .gpu_resources
                .linear_transformed_cosines()
                .bind_group(),
            &[],
        );
        cpass.set_bind_group(6, parameters.gpu_resources.debug_lines().bind_group(), &[]);
        cpass.insert_debug_marker("terrarium::ltc_cull");
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
