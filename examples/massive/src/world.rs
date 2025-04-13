use specs::{Builder, WorldExt};
use terrarium::world::components::{MeshComponent, TransformComponent};
use terrarium::world::transform::Transform;
use uuid::Uuid;

pub struct EntityInfoComponent {
    pub entity_name: String,
    uuid: Uuid,
    marked_for_destroy: bool,
    entity: Option<specs::Entity>,
}

impl EntityInfoComponent {
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn entity(&self) -> specs::Entity {
        self.entity.unwrap()
    }
}

impl specs::Component for EntityInfoComponent {
    type Storage = specs::VecStorage<Self>;
}

pub struct EntityBuilder<'a> {
    builder: specs::EntityBuilder<'a>,
}

impl<'a> EntityBuilder<'a> {
    fn new(
        ecs: &'a mut specs::World,
        name: &str,
        transform: Transform,
        parent: Option<specs::Entity>,
    ) -> Self {
        let entity_info_component = EntityInfoComponent {
            entity_name: name.to_owned(),
            uuid: Uuid::new_v4(),
            marked_for_destroy: false,
            entity: None,
        };
        let transform_component = TransformComponent::new(transform, parent);

        let builder = ecs
            .create_entity()
            .with(entity_info_component)
            .with(transform_component);

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
        ecs.register::<TransformComponent>();

        Self {
            ecs,
            entities_marked_for_destroy: Vec::new(),
        }
    }

    pub fn create_entity<F>(
        &mut self,
        name: &str,
        transform: Transform,
        parent: Option<specs::Entity>,
        mut builder_pattern: F,
    ) -> specs::Entity
    where
        F: FnMut(EntityBuilder<'_>) -> EntityBuilder<'_>,
    {
        let entity = {
            let builder =
                builder_pattern(EntityBuilder::new(&mut self.ecs, name, transform, parent));
            builder.builder.build()
        };

        self.entities_mut::<EntityInfoComponent>()
            .get_mut(entity)
            .unwrap()
            .entity = Some(entity);

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
}
