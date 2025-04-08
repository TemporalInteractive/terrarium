use std::{iter, sync::Arc};

use glam::Vec4;
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
    world::components::{MeshComponent, TransformComponent},
    xr::XrCameraState,
};

const MAX_TLAS_INSTANCES: usize = 1024 * 8;

mod linear_block_allocator;
pub mod material_pool;
pub mod sky;
pub mod vertex_pool;

#[derive(Debug, Clone)]
pub struct GpuMesh {
    pub vertex_pool_alloc: VertexPoolAlloc,
    pub blas: wgpu::Blas,
}

#[derive(Debug, Clone)]
pub struct GpuMaterial {
    pub material_idx: u32,
}

pub struct GpuResources {
    vertex_pool: VertexPool,
    material_pool: MaterialPool,
    tlas_package: wgpu::TlasPackage,
    sky: Sky,

    gpu_meshes: Vec<Arc<GpuMesh>>,
    gpu_materials: Vec<Arc<GpuMaterial>>,
}

impl GpuResources {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_pool = VertexPool::new(device);
        let material_pool = MaterialPool::new(device);

        let tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("terrarium::gpu_resources tlas"),
            max_instances: MAX_TLAS_INSTANCES as u32,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        let sky = Sky::new(device);

        Self {
            vertex_pool,
            material_pool,
            tlas_package: wgpu::TlasPackage::new(tlas),
            sky,
            gpu_meshes: Vec::new(),
            gpu_materials: Vec::new(),
        }
    }

    pub fn create_gpu_mesh(
        &mut self,
        mesh: &Mesh,
        material_base_idx: u32,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
    ) -> Arc<GpuMesh> {
        let vertex_pool_alloc = self.vertex_pool.alloc(
            mesh.packed_vertices.len() as u32,
            mesh.indices.len() as u32,
            material_base_idx,
        );

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

    pub fn tlas(&self) -> &wgpu::Tlas {
        self.tlas_package.tlas()
    }

    pub fn sky(&self) -> &Sky {
        &self.sky
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

    pub fn update(
        &mut self,
        xr_camera_state: &XrCameraState,
        world: &specs::World,
        command_encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
    ) {
        self.cleanup();

        let (transform_storage, mesh_storage): (
            specs::ReadStorage<'_, TransformComponent>,
            specs::ReadStorage<'_, MeshComponent>,
        ) = world.system_data();

        let mut blas_instances: Vec<wgpu::TlasInstance> = vec![];

        for (transform_component, mesh_component) in (&transform_storage, &mesh_storage).join() {
            let mut transform = transform_component.transform.get_matrix();
            transform.w_axis -= Vec4::from((xr_camera_state.stage_translation, 0.0));
            let transform4x3 = transform.transpose().to_cols_array()[..12]
                .try_into()
                .unwrap();

            self.vertex_pool.submit_slice_instance(transform);

            let gpu_mesh = &mesh_component.mesh;
            let blas = &gpu_mesh.blas;
            let vertex_slice_index = gpu_mesh.vertex_pool_alloc.index;

            blas_instances.push(wgpu::TlasInstance::new(
                blas,
                transform4x3,
                vertex_slice_index,
                0xff,
            ));
        }

        let num_blas_instances = blas_instances.len();
        let tlas_package_instances = self
            .tlas_package
            .get_mut_slice(0..MAX_TLAS_INSTANCES)
            .unwrap();
        for (i, instance) in blas_instances.into_iter().enumerate() {
            tlas_package_instances[i] = Some(instance);
        }
        for instance in tlas_package_instances
            .iter_mut()
            .take(MAX_TLAS_INSTANCES)
            .skip(num_blas_instances)
        {
            *instance = None;
        }

        self.vertex_pool.write_slices(queue);
        self.material_pool.write_materials(queue);

        command_encoder
            .build_acceleration_structures(iter::empty(), iter::once(&self.tlas_package));
    }

    pub fn end_frame(&mut self) {
        self.vertex_pool.end_frame();
    }
}
