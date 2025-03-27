use bytemuck::Pod;
use futures::{channel::oneshot, executor::block_on};
use std::{future::IntoFuture, sync::Arc};
use wgpu::{DownlevelCapabilities, Features, Instance, Limits, PowerPreference};
use winit::{
    dpi::PhysicalSize,
    event::{Event, StartCause},
    window::Window,
};

pub mod context;
pub mod pipeline_database;
pub mod surface;

pub use context::Context;
pub use pipeline_database::PipelineDatabase;
pub use surface::Surface;

pub trait ComputePipelineDescriptorExtensions<'a> {
    fn partial_default(module: &'a wgpu::ShaderModule) -> Self;
}

impl<'a> ComputePipelineDescriptorExtensions<'a> for wgpu::ComputePipelineDescriptor<'a> {
    fn partial_default(module: &'a wgpu::ShaderModule) -> Self {
        wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module,
            entry_point: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        }
    }
}

pub async fn readback_buffer_async<T: Pod>(
    staging_buffer: &wgpu::Buffer,
    device: &wgpu::Device,
) -> Vec<T> {
    let buffer_slice = staging_buffer.slice(..);
    let (sender, receiver) = oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    device.poll(wgpu::Maintain::Wait);
    receiver.into_future().await.unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let result = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();
    result
}

pub fn readback_buffer<T: Pod>(staging_buffer: &wgpu::Buffer, device: &wgpu::Device) -> Vec<T> {
    block_on(readback_buffer_async(staging_buffer, device))
}

static EMPTY_TEXTURE_VIEW: std::sync::OnceLock<wgpu::TextureView> = std::sync::OnceLock::new();

pub fn empty_texture_view(device: &wgpu::Device) -> &wgpu::TextureView {
    EMPTY_TEXTURE_VIEW.get_or_init(|| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            label: Some("Empty"),
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        })
    })
}

static EMPTY_BIND_GROUP_LAYOUT: std::sync::OnceLock<wgpu::BindGroupLayout> =
    std::sync::OnceLock::new();

pub fn empty_bind_group_layout(device: &wgpu::Device) -> &wgpu::BindGroupLayout {
    EMPTY_BIND_GROUP_LAYOUT.get_or_init(|| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[],
        })
    })
}

static EMPTY_BIND_GROUP: std::sync::OnceLock<wgpu::BindGroup> = std::sync::OnceLock::new();

pub fn empty_bind_group(device: &wgpu::Device) -> &wgpu::BindGroup {
    EMPTY_BIND_GROUP.get_or_init(|| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: empty_bind_group_layout(device),
            entries: &[],
        })
    })
}
