use std::{iter, sync::Arc};

use debug_lines::DebugLines;
use glam::Vec3;
use material_pool::MaterialPool;
use sky::Sky;
use specs::Join;
use ugm::{
    material::Material,
    mesh::{Mesh, PackedVertex},
    Model,
};
use vertex_pool::{VertexPool, VertexPoolAlloc, VertexPoolWriteData};

use crate::{
    wgpu_util,
    world::components::{DynamicComponent, MeshComponent, TransformComponent},
};

const MAX_STATIC_INSTANCES: usize = 1024 * 256;
const MAX_DYNAMIC_INSTANCES: usize = 1024 * 16;

pub mod debug_lines;
mod linear_block_allocator;
pub mod material_pool;
pub mod sky;
pub mod vertex_pool;

pub struct GpuModel {
    pub gpu_meshes: Vec<Arc<GpuMesh>>,
    pub gpu_materials: Vec<Arc<GpuMaterial>>,
}

impl GpuModel {
    pub fn new(
        model: &Model,
        gpu_resources: &mut GpuResources,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
    ) -> Self {
        let gpu_meshes: Vec<Arc<GpuMesh>> = model
            .meshes
            .iter()
            .map(|mesh| gpu_resources.create_gpu_mesh(mesh, command_encoder, ctx))
            .collect();
        let gpu_materials: Vec<Arc<GpuMaterial>> = model
            .materials
            .iter()
            .map(|material| gpu_resources.create_gpu_material(model, material, ctx))
            .collect();

        Self {
            gpu_meshes,
            gpu_materials,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuMesh {
    pub vertex_pool_alloc: VertexPoolAlloc,
    pub blas: wgpu::Blas,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

#[derive(Debug, Clone)]
pub struct GpuMaterial {
    pub material_idx: u32,
}

pub struct GpuResources {
    vertex_pool: VertexPool,
    material_pool: MaterialPool,
    debug_lines: DebugLines,
    static_tlas_package: wgpu::TlasPackage,
    dynamic_tlas_package: wgpu::TlasPackage,
    static_dirty: bool,
    sky: Sky,

    dynamic_blas_instances: Vec<wgpu::TlasInstance>,
    static_blas_instances: Vec<wgpu::TlasInstance>,

    gpu_meshes: Vec<Arc<GpuMesh>>,
    gpu_materials: Vec<Arc<GpuMaterial>>,
}

impl GpuResources {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_pool = VertexPool::new(device);
        let material_pool = MaterialPool::new(device);
        let debug_lines = DebugLines::new(device);

        let static_tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("terrarium::gpu_resources static_tlas"),
            max_instances: (MAX_STATIC_INSTANCES) as u32,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        let dynamic_tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("terrarium::gpu_resources dynamic_tlas"),
            max_instances: (MAX_DYNAMIC_INSTANCES) as u32,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        let sky = Sky::new(device);

        Self {
            vertex_pool,
            material_pool,
            debug_lines,
            static_tlas_package: wgpu::TlasPackage::new(static_tlas),
            dynamic_tlas_package: wgpu::TlasPackage::new(dynamic_tlas),
            static_dirty: true,
            sky,
            dynamic_blas_instances: Vec::new(),
            static_blas_instances: Vec::new(),
            gpu_meshes: Vec::new(),
            gpu_materials: Vec::new(),
        }
    }

    pub fn create_gpu_mesh(
        &mut self,
        mesh: &Mesh,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
    ) -> Arc<GpuMesh> {
        let vertex_pool_alloc = self
            .vertex_pool
            .alloc(mesh.packed_vertices.len() as u32, mesh.indices.len() as u32);

        self.vertex_pool.write_vertex_data(
            &VertexPoolWriteData {
                packed_vertices: &mesh.packed_vertices,
                indices: &mesh.indices,
                triangle_material_indices: &mesh.triangle_material_indices,
            },
            &vertex_pool_alloc,
            &ctx.queue,
        );

        let size_desc = wgpu::BlasTriangleGeometrySizeDescriptor {
            vertex_format: wgpu::VertexFormat::Float32x3,
            vertex_count: mesh.packed_vertices.len() as u32,
            index_format: Some(wgpu::IndexFormat::Uint32),
            index_count: Some(mesh.indices.len() as u32),
            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
        };

        let blas = ctx.device.create_blas(
            &wgpu::CreateBlasDescriptor {
                label: None,
                flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                update_mode: wgpu::AccelerationStructureUpdateMode::Build,
            },
            wgpu::BlasGeometrySizeDescriptors::Triangles {
                descriptors: vec![size_desc.clone()],
            },
        );

        let triangle_geometry = wgpu::BlasTriangleGeometry {
            size: &size_desc,
            vertex_buffer: self.vertex_pool.vertex_buffer(),
            first_vertex: vertex_pool_alloc.vertex_alloc.start() as u32,
            vertex_stride: std::mem::size_of::<PackedVertex>() as u64,
            index_buffer: Some(self.vertex_pool.index_buffer()),
            first_index: Some(vertex_pool_alloc.index_alloc.start() as u32),
            transform_buffer: None,
            transform_buffer_offset: None,
        };

        let build_entry = wgpu::BlasBuildEntry {
            blas: &blas,
            geometry: wgpu::BlasGeometries::TriangleGeometries(vec![triangle_geometry]),
        };

        command_encoder.build_acceleration_structures(iter::once(&build_entry), iter::empty());

        let gpu_mesh = Arc::new(GpuMesh {
            vertex_pool_alloc,
            blas,
            bounds_min: mesh.bounds_min.into(),
            bounds_max: mesh.bounds_max.into(),
        });
        self.gpu_meshes.push(gpu_mesh.clone());
        gpu_mesh
    }

    pub fn create_gpu_material(
        &mut self,
        model: &Model,
        material: &Material,
        ctx: &wgpu_util::Context,
    ) -> Arc<GpuMaterial> {
        let material_idx =
            self.material_pool
                .alloc_material(model, material, &ctx.device, &ctx.queue);

        let gpu_material = Arc::new(GpuMaterial { material_idx });
        self.gpu_materials.push(gpu_material.clone());
        gpu_material
    }

    pub fn vertex_pool(&self) -> &VertexPool {
        &self.vertex_pool
    }

    pub fn material_pool(&self) -> &MaterialPool {
        &self.material_pool
    }

    pub fn debug_lines(&self) -> &DebugLines {
        &self.debug_lines
    }

    pub fn debug_lines_mut(&mut self) -> &mut DebugLines {
        &mut self.debug_lines
    }

    pub fn static_tlas(&self) -> &wgpu::Tlas {
        self.static_tlas_package.tlas()
    }

    pub fn dynamic_tlas(&self) -> &wgpu::Tlas {
        self.dynamic_tlas_package.tlas()
    }

    pub fn sky(&self) -> &Sky {
        &self.sky
    }

    pub fn sky_mut(&mut self) -> &mut Sky {
        &mut self.sky
    }

    fn cleanup(&mut self) {
        fn vec_remove_multiple<T>(vec: &mut Vec<T>, indices: &mut [usize]) {
            indices.sort();
            for (j, i) in indices.iter().enumerate() {
                vec.remove(i - j);
            }
        }

        let mut gpu_mesh_indices_to_remove = vec![];
        for (i, gpu_mesh) in self.gpu_meshes.iter().enumerate() {
            if Arc::strong_count(gpu_mesh) == 1 {
                gpu_mesh_indices_to_remove.push(i);

                self.vertex_pool.free(&gpu_mesh.vertex_pool_alloc);
            }
        }
        vec_remove_multiple(&mut self.gpu_meshes, &mut gpu_mesh_indices_to_remove);
    }

    pub fn mark_statics_dirty(&mut self) {
        self.static_dirty = true;
    }

    pub fn update(
        &mut self,
        world: &specs::World,
        command_encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
    ) {
        self.cleanup();

        self.static_blas_instances.clear();
        if self.static_dirty {
            let (transform_storage, mesh_storage): (
                specs::ReadStorage<'_, TransformComponent>,
                specs::ReadStorage<'_, MeshComponent>,
            ) = world.system_data();
            for (transform_component, mesh_component) in (&transform_storage, &mesh_storage).join()
            {
                if !mesh_component.enabled || !transform_component.is_static() {
                    continue;
                }

                let transform = transform_component.get_local_to_world_matrix(&transform_storage);
                let transform4x3 = transform.transpose().to_cols_array()[..12]
                    .try_into()
                    .unwrap();

                let gpu_mesh = &mesh_component.mesh;
                let blas = &gpu_mesh.blas;
                let vertex_slice_index = gpu_mesh.vertex_pool_alloc.index;

                let instance_idx = self.vertex_pool.submit_slice_instance(
                    transform,
                    true,
                    vertex_slice_index,
                    &mesh_component.materials,
                );

                let blas_instance = wgpu::TlasInstance::new(blas, transform4x3, instance_idx, 0xff);

                self.static_blas_instances.push(blas_instance);
            }
        }

        self.dynamic_blas_instances.clear();
        {
            let (transform_storage, mesh_storage, dynamic_storage): (
                specs::ReadStorage<'_, TransformComponent>,
                specs::ReadStorage<'_, MeshComponent>,
                specs::ReadStorage<'_, DynamicComponent>,
            ) = world.system_data();
            for (transform_component, mesh_component, _) in
                (&transform_storage, &mesh_storage, &dynamic_storage).join()
            {
                assert!(!transform_component.is_static(), "Detected a static TransformComponent on an entity containing the DynamicComponent!");
                if !mesh_component.enabled {
                    continue;
                }

                let transform = transform_component.get_local_to_world_matrix(&transform_storage);
                let transform4x3 = transform.transpose().to_cols_array()[..12]
                    .try_into()
                    .unwrap();

                let gpu_mesh = &mesh_component.mesh;
                let blas = &gpu_mesh.blas;
                let vertex_slice_index = gpu_mesh.vertex_pool_alloc.index;

                let instance_idx = self.vertex_pool.submit_slice_instance(
                    transform,
                    false,
                    vertex_slice_index,
                    &mesh_component.materials,
                );

                let blas_instance = wgpu::TlasInstance::new(blas, transform4x3, instance_idx, 0xff);

                self.dynamic_blas_instances.push(blas_instance);
            }
        }

        let num_blas_instances = self.dynamic_blas_instances.len();
        assert!(num_blas_instances <= MAX_DYNAMIC_INSTANCES);
        let tlas_package_instances = self
            .dynamic_tlas_package
            .get_mut_slice(0..MAX_DYNAMIC_INSTANCES)
            .unwrap();
        for (i, instance) in self.dynamic_blas_instances.iter().enumerate() {
            tlas_package_instances[i] = Some(instance.clone());
        }
        for instance in tlas_package_instances.iter_mut().skip(num_blas_instances) {
            *instance = None;
        }

        if self.static_dirty {
            let num_blas_instances = self.static_blas_instances.len();
            assert!(num_blas_instances <= MAX_STATIC_INSTANCES);
            let tlas_package_instances = self
                .static_tlas_package
                .get_mut_slice(0..MAX_STATIC_INSTANCES)
                .unwrap();
            for (i, instance) in self.static_blas_instances.iter().enumerate() {
                tlas_package_instances[i] = Some(instance.clone());
            }
            for instance in tlas_package_instances.iter_mut().skip(num_blas_instances) {
                *instance = None;
            }
        }

        self.vertex_pool.write_slices(queue);
        self.material_pool.write_materials(queue);
        self.debug_lines.write_lines(queue);

        let mut tlases = vec![&self.dynamic_tlas_package];
        if self.static_dirty {
            tlases.push(&self.static_tlas_package);
            self.static_dirty = false;
        }
        command_encoder.build_acceleration_structures(iter::empty(), tlases);
    }

    pub fn end_frame(&mut self) {
        self.vertex_pool.end_frame();
        self.debug_lines.end_frame();
    }
}
