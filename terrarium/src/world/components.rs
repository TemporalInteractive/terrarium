use std::sync::OnceLock;

use ugm::mesh::Mesh;

use crate::gpu_resources::GpuMesh;

use super::transform::Transform;

pub struct TransformComponent {
    pub transform: Transform,
}

impl TransformComponent {
    pub fn new(transform: Transform) -> Self {
        Self { transform }
    }
}

impl specs::Component for TransformComponent {
    type Storage = specs::VecStorage<Self>;
}

#[derive(Debug)]
pub struct MeshComponent {
    pub(crate) mesh: GpuMesh,
}

impl MeshComponent {
    pub fn new(mesh: GpuMesh) -> Self {
        Self { mesh }
    }
}

impl specs::Component for MeshComponent {
    type Storage = specs::VecStorage<Self>;
}
