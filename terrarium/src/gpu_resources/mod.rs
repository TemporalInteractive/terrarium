use specs::Join;
use ugm::mesh::Mesh;
use vertex_pool::{VertexPool, VertexPoolAlloc, VertexPoolWriteData};

use crate::{
    wgpu_util,
    world::components::{MeshComponent, TransformComponent},
};

pub mod vertex_pool;

#[derive(Debug, Clone)]
pub struct GpuMesh {
    pub vertex_pool_alloc: VertexPoolAlloc,
}

impl GpuMesh {
    pub fn new(mesh: &Mesh, gpu_resources: &mut GpuResources, ctx: &wgpu_util::Context) -> Self {
        let vertex_pool_alloc = gpu_resources.vertex_pool.alloc(
            mesh.packed_vertices.len() as u32,
            mesh.indices.len() as u32,
            0,
        );

        gpu_resources.vertex_pool.write_vertex_data(
            &VertexPoolWriteData {
                packed_vertices: &mesh.packed_vertices,
                indices: &mesh.indices,
                triangle_material_indices: &mesh.triangle_material_indices,
            },
            vertex_pool_alloc.slice,
            &ctx.queue,
        );

        Self { vertex_pool_alloc }
    }
}

pub struct GpuResources {
    vertex_pool: VertexPool,
}

impl GpuResources {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_pool = VertexPool::new(device);

        Self { vertex_pool }
    }

    pub fn vertex_pool(&self) -> &VertexPool {
        &self.vertex_pool
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
    }

    pub fn end_frame(&mut self) {
        self.vertex_pool.end_frame();
    }
}
