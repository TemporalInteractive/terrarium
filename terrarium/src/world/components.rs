use std::sync::{Arc, Mutex};

use glam::{Mat4, Quat, Vec3};

use crate::gpu_resources::{GpuMaterial, GpuMesh};

use super::transform::Transform;

pub struct TransformComponent {
    global_transform: Mutex<(Mat4, bool)>,
    local_transform: Transform,
    pub parent: Option<specs::Entity>,
    pub children: Vec<specs::Entity>,
}

impl TransformComponent {
    pub fn new(local_transform: Transform) -> Self {
        let global_transform = Mutex::new((local_transform.get_matrix(), true));

        Self {
            global_transform,
            local_transform,
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn get_translation(&self, transforms: &specs::ReadStorage<'_, TransformComponent>) -> Vec3 {
        self.resolve_global_transform(transforms);

        let (_scale, _rotation, translation) = self
            .resolve_global_transform(transforms)
            .to_scale_rotation_translation();
        translation
    }

    pub fn get_rotation(&self, transforms: &specs::ReadStorage<'_, TransformComponent>) -> Quat {
        let (_scale, rotation, _translation) = self
            .resolve_global_transform(transforms)
            .to_scale_rotation_translation();
        rotation
    }

    pub fn get_local_translation(&self) -> Vec3 {
        self.local_transform.get_translation()
    }

    pub fn set_local_translation(&mut self, translation: Vec3) {
        self.local_transform.set_translation(translation);
    }

    pub fn translate_local(&mut self, translation: Vec3) {
        self.local_transform.translate(translation);
    }

    pub fn get_local_rotation(&self) -> Quat {
        self.local_transform.get_rotation()
    }

    pub fn set_local_rotation(&mut self, rotation: Quat) {
        self.local_transform.set_rotation(rotation);
    }

    pub fn rotate_local(&mut self, rotation: Quat) {
        self.local_transform.rotate(rotation);
    }

    pub fn get_local_scale(&self) -> Vec3 {
        self.local_transform.get_scale()
    }

    pub fn get_local_to_world_matrix(
        &self,
        transforms: &specs::ReadStorage<'_, TransformComponent>,
    ) -> Mat4 {
        self.resolve_global_transform(transforms)
    }

    pub fn get_local_to_local_matrix(&self) -> Mat4 {
        self.local_transform.get_matrix()
    }

    fn resolve_global_transform(
        &self,
        transforms: &specs::ReadStorage<'_, TransformComponent>,
    ) -> Mat4 {
        if self.parent.is_none() {
            return self.local_transform.get_matrix();
        }

        let mut matrix = self.global_transform.lock().unwrap();

        if matrix.1 {
            let mut global_transform = self.local_transform.get_matrix();

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

        matrix.0
    }

    pub fn mark_dirty(&self, transforms: &specs::ReadStorage<'_, TransformComponent>) {
        self.global_transform.lock().unwrap().1 = true;

        for child in &self.children {
            let child_transform = transforms.get(*child).unwrap();
            child_transform.mark_dirty(transforms);
        }
    }
}

impl specs::Component for TransformComponent {
    type Storage = specs::VecStorage<Self>;
}

#[derive(Debug)]
pub struct MeshComponent {
    pub enabled: bool,
    pub mesh: Arc<GpuMesh>,
    pub materials: Vec<Arc<GpuMaterial>>,
}

impl MeshComponent {
    pub fn new(mesh: Arc<GpuMesh>, materials: Vec<Arc<GpuMaterial>>) -> Self {
        Self {
            enabled: true,
            mesh,
            materials,
        }
    }
}

impl specs::Component for MeshComponent {
    type Storage = specs::VecStorage<Self>;
}
