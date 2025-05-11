use glam::UVec2;

pub struct Gbuffer {
    position_and_depth_texture: wgpu::Texture,
    shading_and_geometric_normal_texture: wgpu::Texture,
    tex_coord_and_derivatives_texture: wgpu::Texture,
    velocity_texture: wgpu::Texture,
    material_descriptor_idx_and_normal_roughness_texture: wgpu::Texture,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl Gbuffer {
    pub fn new(resolution: UVec2, device: &wgpu::Device) -> Self {
        let create_texture = |name, format| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("terrarium::gbuffer {}", name)),
                size: wgpu::Extent3d {
                    width: resolution.x,
                    height: resolution.y,
                    depth_or_array_layers: 2,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: Some(2),
                ..Default::default()
            });

            (texture, texture_view)
        };

        let position_and_depth_texture =
            create_texture("position_and_depth", wgpu::TextureFormat::Rgba32Float);
        let shading_and_geometric_normal_texture = create_texture(
            "shading_and_geometric_normal",
            wgpu::TextureFormat::Rg32Uint,
        );
        let tex_coord_and_derivatives_texture = create_texture(
            "tex_coord_and_derivatives",
            wgpu::TextureFormat::Rgba32Float,
        );
        let velocity_texture = create_texture("velocity", wgpu::TextureFormat::Rg32Float);
        let material_descriptor_idx_and_normal_roughness_texture = create_texture(
            "material_descriptor_idx_and_normal_roughness",
            wgpu::TextureFormat::Rg32Float,
        );

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rg32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rg32Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rg32Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&position_and_depth_texture.1),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &shading_and_geometric_normal_texture.1,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &tex_coord_and_derivatives_texture.1,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&velocity_texture.1),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &material_descriptor_idx_and_normal_roughness_texture.1,
                    ),
                },
            ],
        });

        Self {
            position_and_depth_texture: position_and_depth_texture.0,
            shading_and_geometric_normal_texture: shading_and_geometric_normal_texture.0,
            tex_coord_and_derivatives_texture: tex_coord_and_derivatives_texture.0,
            velocity_texture: velocity_texture.0,
            material_descriptor_idx_and_normal_roughness_texture:
                material_descriptor_idx_and_normal_roughness_texture.0,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
