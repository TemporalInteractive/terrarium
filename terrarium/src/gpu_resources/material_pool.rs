use std::{collections::HashMap, num::NonZeroU32};

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use ugm::{material::Material, texture::Texture, Model};
use uuid::Uuid;

use crate::wgpu_util::empty_texture_view;

pub const MAX_MATERIAL_POOL_MATERIALS: usize = 1024 * 8;
pub const MAX_MATERIAL_POOL_TEXTURES: usize = 1024;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct TextureTransform {
    uv_offset: Vec2,
    uv_scale: Vec2,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct MaterialDescriptor {
    pub color: Vec3,
    color_texture: u32,
    pub metallic: f32,
    pub roughness: f32,
    metallic_roughness_texture: u32,
    pub normal_scale: f32,
    pub emission: Vec3,
    pub normal_texture: u32,
    emission_texture: u32,
    pub transmission: f32,
    pub eta: f32,
    pub subsurface: f32,
    pub absorption: Vec3,
    pub specular: f32,
    pub specular_tint: Vec3,
    pub anisotropic: f32,
    pub sheen: f32,
    sheen_texture: u32,
    pub clearcoat: f32,
    pub clearcoat_texture: u32,
    pub clearcoat_roughness: f32,
    clearcoat_roughness_texture: u32,
    pub alpha_cutoff: f32,
    sheen_tint_texture: u32,
    clearcoat_normal_texture: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
    pub sheen_tint: Vec3,
    transmission_texture: u32,
}

pub struct MaterialPool {
    material_descriptor_buffer: wgpu::Buffer,
    texture_transform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    texture_views: Vec<wgpu::TextureView>,
    texture_indices: HashMap<Uuid, usize>,

    material_descriptors: Vec<MaterialDescriptor>,
    texture_transforms: Vec<TextureTransform>,

    bind_group_layout: wgpu::BindGroupLayout,
}

impl MaterialPool {
    pub fn new(device: &wgpu::Device) -> Self {
        let material_descriptor_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::material_pool material_descriptors"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<MaterialDescriptor>() * MAX_MATERIAL_POOL_MATERIALS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let texture_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::material_pool texture_transforms"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<TextureTransform>() * MAX_MATERIAL_POOL_MATERIALS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: Some(NonZeroU32::new(MAX_MATERIAL_POOL_TEXTURES as u32).unwrap()),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        Self {
            material_descriptor_buffer,
            texture_transform_buffer,
            sampler,
            texture_views: Vec::new(),
            texture_indices: HashMap::new(),

            material_descriptors: Vec::new(),
            texture_transforms: Vec::new(),
            bind_group_layout,
        }
    }

    fn alloc_texture(
        &mut self,
        model_texture: &Texture,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> u32 {
        let (_texture, texture_view) = model_texture.create_wgpu_texture(
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            srgb,
            device,
            queue,
        );

        self.texture_views.push(texture_view);
        let texture_idx = self.texture_views.len() - 1;

        self.texture_transforms.push(TextureTransform {
            uv_offset: model_texture.uv_offset().into(),
            uv_scale: model_texture.uv_scale().into(),
        });

        self.texture_indices
            .insert(model_texture.uuid(), texture_idx);
        texture_idx as u32
    }

    pub fn material_count(&self) -> usize {
        self.material_descriptors.len()
    }

    pub fn material_descriptor(&self, i: u32) -> &MaterialDescriptor {
        &self.material_descriptors[i as usize]
    }

    pub fn alloc_material(
        &mut self,
        model: &Model,
        material: &Material,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> u32 {
        let color_texture = if let Some(texture_idx) = &material.color_texture {
            let texture = &model.textures[*texture_idx as usize];

            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, true, device, queue)
            }
        } else {
            u32::MAX
        };

        let metallic_roughness_texture =
            if let Some(texture_idx) = &material.metallic_roughness_texture {
                let texture = &model.textures[*texture_idx as usize];

                if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                    *texture_idx as u32
                } else {
                    self.alloc_texture(texture, false, device, queue)
                }
            } else {
                u32::MAX
            };

        let normal_texture = if let Some(texture_idx) = &material.normal_texture {
            let texture = &model.textures[*texture_idx as usize];

            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, false, device, queue)
            }
        } else {
            u32::MAX
        };

        let emission_texture = if let Some(texture_idx) = &material.emission_texture {
            let texture = &model.textures[*texture_idx as usize];

            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, true, device, queue)
            }
        } else {
            u32::MAX
        };

        let material_descriptor = MaterialDescriptor {
            color: material.color.into(),
            color_texture,
            metallic: material.metallic,
            roughness: material.roughness,
            metallic_roughness_texture,
            normal_scale: material.normal_scale,
            emission: material.emission.into(),
            normal_texture,
            emission_texture,
            transmission: material.transmission,
            transmission_texture: u32::MAX,
            eta: material.eta,
            subsurface: material.subsurface,
            absorption: material.absorption.into(),
            specular: material.specular,
            specular_tint: material.specular_tint.into(),
            anisotropic: material.anisotropic,
            sheen: material.sheen,
            sheen_tint: material.sheen_tint.into(),
            clearcoat: material.clearcoat,
            clearcoat_texture: u32::MAX,
            clearcoat_roughness: material.clearcoat_roughness,
            clearcoat_roughness_texture: u32::MAX,
            alpha_cutoff: material.alpha_cutoff,
            sheen_texture: u32::MAX,
            clearcoat_normal_texture: u32::MAX,
            sheen_tint_texture: u32::MAX,
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
        };

        self.material_descriptors.push(material_descriptor);
        self.material_descriptors.len() as u32 - 1
    }

    pub fn write_materials(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.material_descriptor_buffer,
            0,
            bytemuck::cast_slice(self.material_descriptors.as_slice()),
        );
        queue.write_buffer(
            &self.texture_transform_buffer,
            0,
            bytemuck::cast_slice(self.texture_transforms.as_slice()),
        );
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group<F>(
        &self,
        bind_group_layout: wgpu::BindGroupLayout,
        device: &wgpu::Device,
        mut callback: F,
    ) where
        F: FnMut(&wgpu::BindGroup),
    {
        let mut entries = vec![];
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.material_descriptor_buffer.as_entire_binding(),
        });

        let mut texture_views = vec![];
        for texture in &self.texture_views {
            texture_views.push(texture);
        }
        for _ in 0..(MAX_MATERIAL_POOL_TEXTURES - self.texture_views.len()) {
            texture_views.push(empty_texture_view(device));
        }

        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureViewArray(&texture_views),
        });

        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: self.texture_transform_buffer.as_entire_binding(),
        });

        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &entries,
        });

        callback(&bind_group);
    }
}
