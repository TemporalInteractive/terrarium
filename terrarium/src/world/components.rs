use std::sync::{Arc, Mutex};

use glam::{Mat4, Vec3};

use crate::gpu_resources::{GpuMaterial, GpuMesh};

use super::transform::Transform;

pub struct TransformComponent {
    global_transform: Mutex<(Mat4, bool)>,
    local_transform: Transform,
    parent: Option<specs::Entity>,
}

impl TransformComponent {
    pub fn new(local_transform: Transform, parent: Option<specs::Entity>) -> Self {
        let global_transform = Mutex::new((local_transform.get_matrix(), parent.is_some()));

        Self {
            global_transform,
            local_transform,
            parent,
        }
    }

    // pub fn get_position(&self) -> Vec3 {
    //     self.global_transform.get_translation()
    // }

    // pub fn set_position(&mut self, position: Vec3) {
    //     self.global_transform.set_translation(position);
    // }

    // pub fn translate(&mut self, translation: Vec3) {
    //     self.global_transform.translate(translation);
    // }

    pub fn get_local_to_world_matrix(
        &self,
        transforms: &specs::ReadStorage<'_, TransformComponent>,
    ) -> Mat4 {
        self.resolve_global_transform(false, transforms);

        self.global_transform.lock().unwrap().0
    }

    pub fn get_world_to_view_matrix(
        &self,
        transforms: &specs::ReadStorage<'_, TransformComponent>,
    ) -> Mat4 {
        self.resolve_global_transform(true, transforms);

        self.global_transform.lock().unwrap().0
    }

    fn resolve_global_transform(
        &self,
        is_view: bool,
        transforms: &specs::ReadStorage<'_, TransformComponent>,
    ) {
        let mut matrix = self.global_transform.lock().unwrap();

        if matrix.1 {
            let mut global_transform = if is_view {
                self.local_transform.get_matrix()
            } else {
                self.local_transform.get_view_matrix()
            };

            let mut optional_parent = self.parent;
            while let Some(parent) = optional_parent {
                let parent_transform = transforms.get(parent).unwrap();

                let parent_local_transform = parent_transform.local_transform.get_matrix();
                global_transform = parent_local_transform * global_transform;

                optional_parent = parent_transform.parent;
            }

            matrix.0 = global_transform;
            matrix.1 = false;
        }
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
