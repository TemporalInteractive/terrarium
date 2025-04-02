use std::iter;

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
};

const MAX_TLAS_INSTANCES: usize = 1024 * 8;

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
        }
    }

    pub fn create_gpu_mesh(
        &mut self,
        mesh: &Mesh,
        material_base_idx: u32,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
    ) -> GpuMesh {
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
            vertex_pool_alloc.slice,
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
            first_vertex: vertex_pool_alloc.slice.first_vertex(),
            vertex_stride: std::mem::size_of::<PackedVertex>() as u64,
            index_buffer: Some(self.vertex_pool.index_buffer()),
            first_index: Some(vertex_pool_alloc.slice.first_index()),
            transform_buffer: None,
            transform_buffer_offset: None,
        };

        let build_entry = wgpu::BlasBuildEntry {
            blas: &blas,
            geometry: wgpu::BlasGeometries::TriangleGeometries(vec![triangle_geometry]),
        };

        command_encoder.build_acceleration_structures(iter::once(&build_entry), iter::empty());

        GpuMesh {
            vertex_pool_alloc,
            blas,
        }
    }

    pub fn create_gpu_material(
        &mut self,
        model: &Model,
        material: &Material,
        ctx: &wgpu_util::Context,
    ) -> GpuMaterial {
        let material_idx =
            self.material_pool
                .alloc_material(model, material, &ctx.device, &ctx.queue);

        GpuMaterial { material_idx }
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

    pub fn update(
        &mut self,
        world: &specs::World,
        command_encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
    ) {
        let (transform_storage, mesh_storage): (
            specs::ReadStorage<'_, TransformComponent>,
            specs::ReadStorage<'_, MeshComponent>,
        ) = world.system_data();

        let mut blas_instances: Vec<wgpu::TlasInstance> = vec![];

        for (transform_component, mesh_component) in (&transform_storage, &mesh_storage).join() {
            self.vertex_pool.submit_slice_instance(
                mesh_component.mesh.vertex_pool_alloc.index,
                transform_component.transform.get_matrix(),
                false,
            );

            let transform = transform_component.transform.get_matrix();
            let transform4x3 = transform.transpose().to_cols_array()[..12]
                .try_into()
                .unwrap();

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
        for i in num_blas_instances..MAX_TLAS_INSTANCES {
            tlas_package_instances[i] = None;
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
