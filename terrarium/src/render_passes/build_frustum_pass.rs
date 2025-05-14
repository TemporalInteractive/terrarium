use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec3};
use wgpu::util::DeviceExt;
use wgsl_includes::include_wgsl;

use crate::{
    gpu_resources::gbuffer::Gbuffer,
    wgpu_util::{
        empty_bind_group, empty_bind_group_layout, ComputePipelineDescriptorExtensions,
        PipelineDatabase,
    },
};

pub const TILE_SIZE: u32 = 16;

#[repr(C)]
struct Plane {
    normal: Vec3,
    distance: f32,
}

#[repr(C)]
struct Frustum {
    left: Plane,
    right: Plane,
    top: Plane,
    bottom: Plane,
    near: Plane,
    far: Plane,
}

pub fn create_frustum_buffer(resolution: UVec2, device: &wgpu::Device) -> wgpu::Buffer {
    let num_groups = (resolution.x.div_ceil(TILE_SIZE) * resolution.y.div_ceil(TILE_SIZE)) as usize;

    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrarium::build_frustum_pass frustums"),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        size: (size_of::<Frustum>() * num_groups) as u64 * 2,
        mapped_at_creation: false,
    })
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    tile_resolution: UVec2,
}

pub struct BuildFrustumPassParameters<'a> {
    pub resolution: UVec2,
    pub gbuffer: &'a Gbuffer,
    pub xr_camera_buffer: &'a wgpu::Buffer,
    pub frustum_buffer: &'a wgpu::Buffer,
}

pub fn encode(
    parameters: &BuildFrustumPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_wgsl!("../../shaders/build_frustum_pass.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("terrarium::build_frustum"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrarium::build_frustum"),
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
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    }),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    parameters.gbuffer.bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let tile_resolution = UVec2::new(
        parameters.resolution.x.div_ceil(TILE_SIZE),
        parameters.resolution.y.div_ceil(TILE_SIZE),
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrarium::build_frustum constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            tile_resolution,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
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
                binding: 2,
                resource: parameters.frustum_buffer.as_entire_binding(),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrarium::build_frustum"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_bind_group(1, empty_bind_group(device), &[]);
        cpass.set_bind_group(2, empty_bind_group(device), &[]);
        cpass.set_bind_group(3, empty_bind_group(device), &[]);
        cpass.set_bind_group(4, parameters.gbuffer.bind_group(), &[]);
        cpass.insert_debug_marker("terrarium::build_frustum");
        cpass.dispatch_workgroups(
            parameters.resolution.x.div_ceil(TILE_SIZE),
            parameters.resolution.y.div_ceil(TILE_SIZE),
            1,
        );
    }
}
