#![allow(dead_code)]

use std::sync::Arc;

use glam::{Mat4, Vec3};
use rand::Rng;
use specs::{Builder, WorldExt};
use terrarium::gpu_resources::{GpuMaterial, GpuMesh, GpuResources};
use terrarium::wgpu_util;
use terrarium::world::components::{
    AreaLightComponent, DynamicComponent, MeshComponent, TransformComponent,
};
use terrarium::world::transform::Transform;
use ugm::Model;

pub struct EntityInfoComponent {
    marked_for_destroy: bool,
}

impl specs::Component for EntityInfoComponent {
    type Storage = specs::VecStorage<Self>;
}

pub struct EntityBuilder<'a> {
    builder: specs::EntityBuilder<'a>,
}

impl<'a> EntityBuilder<'a> {
    fn new(ecs: &'a mut specs::World, transform: Transform, is_static: bool) -> Self {
        let entity_info_component = EntityInfoComponent {
            marked_for_destroy: false,
        };
        let transform_component = TransformComponent::new(transform, is_static);

        let mut builder = ecs
            .create_entity()
            .with(entity_info_component)
            .with(transform_component);

        if !is_static {
            builder = builder.with(DynamicComponent);
        }

        Self { builder }
    }

    pub fn with<T: specs::Component + Send + Sync>(self, c: T) -> Self {
        Self {
            builder: self.builder.with(c),
        }
    }
}

pub struct World {
    ecs: specs::World,
    entities_marked_for_destroy: Vec<specs::Entity>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        let mut ecs = specs::World::new();
        ecs.register::<EntityInfoComponent>();
        ecs.register::<MeshComponent>();
        ecs.register::<AreaLightComponent>();
        ecs.register::<TransformComponent>();
        ecs.register::<DynamicComponent>();

        Self {
            ecs,
            entities_marked_for_destroy: Vec::new(),
        }
    }

    pub fn create_entity<F>(
        &mut self,
        transform: Transform,
        is_static: bool,
        parent: Option<specs::Entity>,
        mut builder_pattern: F,
    ) -> specs::Entity
    where
        F: FnMut(EntityBuilder<'_>) -> EntityBuilder<'_>,
    {
        let entity = {
            let builder = builder_pattern(EntityBuilder::new(&mut self.ecs, transform, is_static));
            builder.builder.build()
        };

        entity.set_parent(parent, self);

        entity
    }

    pub fn destroy_entity(&mut self, entity: specs::Entity) {
        self.entities_mut::<EntityInfoComponent>()
            .get_mut(entity)
            .unwrap()
            .marked_for_destroy = true;
        self.entities_marked_for_destroy.push(entity);
    }

    pub fn entities<T: specs::Component>(&self) -> specs::ReadStorage<T> {
        self.ecs.read_storage()
    }

    pub fn entities_mut<T: specs::Component>(&self) -> specs::WriteStorage<T> {
        self.ecs.write_storage()
    }

    pub fn update(&mut self) {
        for entity in &self.entities_marked_for_destroy {
            self.ecs.delete_entity(*entity).unwrap();
        }
        self.entities_marked_for_destroy.clear();
    }

    pub fn specs(&self) -> &specs::World {
        &self.ecs
    }

    fn spawn_model_recursive(
        &mut self,
        model: &Model,
        is_static: bool,
        node: u32,
        parent: specs::Entity,
        gpu_meshes: &[Arc<GpuMesh>],
        gpu_materials: &Vec<Arc<GpuMaterial>>,
    ) {
        let node = &model.nodes[node as usize];
        let transform = Mat4::from_cols_array(&node.transform);

        let entity = self.create_entity(
            Transform::from(transform),
            is_static,
            Some(parent),
            |builder| {
                if let Some(mesh_idx) = node.mesh_idx {
                    let mut used_gpu_materials = Vec::new();
                    for material_idx in &model.meshes[mesh_idx as usize].material_indices {
                        used_gpu_materials.push(gpu_materials[*material_idx as usize].clone());
                    }

                    let mut builder = builder.with(MeshComponent::new(
                        gpu_meshes[mesh_idx as usize].clone(),
                        used_gpu_materials.clone(),
                    ));

                    if model.meshes[mesh_idx as usize].is_emissive {
                        let mut rng = rand::rng();
                        let color = Vec3::new(rng.random(), rng.random(), rng.random());
                        builder = builder.with(AreaLightComponent::new(color, 100.0, 1.0, false));
                    }

                    builder
                } else {
                    builder
                }
            },
        );

        for child_node in &node.child_node_indices {
            self.spawn_model_recursive(
                model,
                is_static,
                *child_node,
                entity,
                gpu_meshes,
                gpu_materials,
            );
        }
    }

    pub fn spawn_model(
        &mut self,
        model: &Model,
        root_transform: Transform,
        is_static: bool,
        parent: Option<specs::Entity>,
        gpu_resources: &mut GpuResources,
        command_encoder: &mut wgpu::CommandEncoder,
        ctx: &wgpu_util::Context,
    ) -> specs::Entity {
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

        let root = self.create_entity(root_transform, is_static, parent, |builder| builder);

        for root_node in &model.root_node_indices {
            self.spawn_model_recursive(
                model,
                is_static,
                *root_node,
                root,
                &gpu_meshes,
                &gpu_materials,
            );
        }

        root
    }
}

pub trait EntityExt {
    fn set_parent(&self, parent: Option<specs::Entity>, world: &mut World);
}
impl EntityExt for specs::Entity {
    fn set_parent(&self, parent: Option<specs::Entity>, world: &mut World) {
        let mut transforms = world.entities_mut::<TransformComponent>();

        if let Some(parent) = transforms.get(*self).unwrap().parent {
            let parent_transform = transforms.get_mut(parent).unwrap();
            parent_transform.children.remove(
                parent_transform
                    .children
                    .iter()
                    .position(|x| *x == *self)
                    .unwrap(),
            );
        }

        transforms.get_mut(*self).unwrap().parent = parent;
        if let Some(parent) = transforms.get(*self).unwrap().parent {
            let parent_transform = transforms.get_mut(parent).unwrap();
            parent_transform.children.push(*self);
        }
    }
}
