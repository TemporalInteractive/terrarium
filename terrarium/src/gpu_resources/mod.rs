use material_pool::MaterialPool;
use specs::Join;
use ugm::{material::Material, mesh::Mesh, Model};
use vertex_pool::{VertexPool, VertexPoolAlloc, VertexPoolWriteData};

use crate::{
    wgpu_util,
    world::components::{MeshComponent, TransformComponent},
};

pub mod material_pool;
pub mod vertex_pool;

#[derive(Debug, Clone)]
pub struct GpuMesh {
    pub vertex_pool_alloc: VertexPoolAlloc,
}

#[derive(Debug, Clone)]
pub struct GpuMaterial {
    pub material_idx: u32,
}

pub struct GpuResources {
    vertex_pool: VertexPool,
    material_pool: MaterialPool,
}

impl GpuResources {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_pool = VertexPool::new(device);
        let material_pool = MaterialPool::new(device);

        Self {
            vertex_pool,
            material_pool,
        }
    }

    pub fn create_gpu_mesh(&mut self, mesh: &Mesh, ctx: &wgpu_util::Context) -> GpuMesh {
        let vertex_pool_alloc = self.vertex_pool.alloc(
            mesh.packed_vertices.len() as u32,
            mesh.indices.len() as u32,
            0,
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

        GpuMesh { vertex_pool_alloc }
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

    pub fn submit_instances(&mut self, world: &specs::World, queue: &wgpu::Queue) {
        let (transform_storage, mesh_storage): (
            specs::ReadStorage<'_, TransformComponent>,
            specs::ReadStorage<'_, MeshComponent>,
        ) = world.system_data();

        for (transform_component, mesh_component) in (&transform_storage, &mesh_storage).join() {
            self.vertex_pool.submit_slice_instance(
                mesh_component.mesh.vertex_pool_alloc.index,
                transform_component.transform.get_matrix(),
                false,
            );
        }

        self.vertex_pool.write_slices(queue);
        self.material_pool.write_materials(queue);
    }

    pub fn end_frame(&mut self) {
        self.vertex_pool.end_frame();
    }
}
