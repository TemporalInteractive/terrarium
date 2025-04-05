use std::sync::Arc;

use crate::gpu_resources::{GpuMaterial, GpuMesh};

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
    pub(crate) mesh: Arc<GpuMesh>,
    pub(crate) materials: Vec<Arc<GpuMaterial>>,
}

impl MeshComponent {
    pub fn new(mesh: Arc<GpuMesh>, materials: Vec<Arc<GpuMaterial>>) -> Self {
        Self { mesh, materials }
    }
}

impl specs::Component for MeshComponent {
    type Storage = specs::VecStorage<Self>;
}
