use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use ugm::mesh::PackedVertex;

use super::{
    linear_block_allocator::{LinearBlockAllocation, LinearBlockAllocator},
    GpuMaterial, MAX_DYNAMIC_INSTANCES, MAX_STATIC_INSTANCES,
};

const MAX_VERTEX_POOL_VERTICES: usize = 1024 * 1024 * 32;
const MAX_VERTEX_POOL_INDICES: usize = 1024 * 1024 * 256;
const MAX_VERTEX_POOL_SLICES: usize = 1024 * 8;
const MAX_MATERIALS_PER_INSTANCE: usize = 8 * 10;

pub struct VertexPoolWriteData<'a> {
    pub packed_vertices: &'a [PackedVertex],
    pub indices: &'a [u32],
    pub triangle_material_indices: &'a [u32],
}

#[derive(Debug, Clone)]
pub struct VertexPoolAlloc {
    pub vertex_alloc: LinearBlockAllocation,
    pub index_alloc: LinearBlockAllocation,
    pub index: u32,
}

#[derive(Pod, Debug, Clone, Copy, Zeroable, PartialEq, Eq)]
#[repr(C)]
pub struct VertexPoolSlice {
    first_vertex: u32,
    num_vertices: u32,
    first_index: u32,
    num_indices: u32,
}

impl VertexPoolSlice {
    fn new(first_vertex: u32, num_vertices: u32, first_index: u32, num_indices: u32) -> Self {
        Self {
            first_vertex,
            num_vertices,
            first_index,
            num_indices,
        }
    }

    fn new_unallocated() -> Self {
        Self {
            first_vertex: 0,
            num_vertices: 0,
            first_index: 0,
            num_indices: 0,
        }
    }

    fn is_allocated(&self) -> bool {
        self.num_vertices > 0
    }
}

pub struct VertexPool {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    triangle_material_index_buffer: wgpu::Buffer,
    slices_buffer: wgpu::Buffer,
    object_to_world_buffer: wgpu::Buffer,
    material_index_buffer: wgpu::Buffer,

    vertex_allocator: LinearBlockAllocator,
    index_allocator: LinearBlockAllocator,
    slices: Box<[VertexPoolSlice]>,
    delta_object_to_world_inv: Vec<Mat4>,
    prev_object_to_world: Vec<Mat4>,
    static_material_indices: Vec<u32>,
    dynamic_material_indices: Vec<u32>,
    frame_idx: u32,

    bind_group_layout: wgpu::BindGroupLayout,
}

impl VertexPool {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool vertices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<PackedVertex>() * MAX_VERTEX_POOL_VERTICES) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool indices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<u32>() * MAX_VERTEX_POOL_INDICES) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT
                | wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let triangle_material_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool triangle_material_indices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<u32>() * MAX_VERTEX_POOL_INDICES / 3) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let slices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool slices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<VertexPoolSlice>() * MAX_VERTEX_POOL_SLICES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let object_to_world_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool object_to_world"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<Mat4>() * (MAX_DYNAMIC_INSTANCES + MAX_STATIC_INSTANCES))
                as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let material_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool material_indices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<u32>()
                * (MAX_DYNAMIC_INSTANCES + MAX_STATIC_INSTANCES)
                * MAX_MATERIALS_PER_INSTANCE) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let vertex_allocator = LinearBlockAllocator::new(MAX_VERTEX_POOL_VERTICES as u64);
        let index_allocator = LinearBlockAllocator::new(MAX_VERTEX_POOL_INDICES as u64);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        Self {
            vertex_buffer,
            index_buffer,
            triangle_material_index_buffer,
            slices_buffer,
            object_to_world_buffer,
            material_index_buffer,

            vertex_allocator,
            index_allocator,
            slices: vec![VertexPoolSlice::new_unallocated(); MAX_VERTEX_POOL_SLICES]
                .into_boxed_slice(),
            delta_object_to_world_inv: Vec::new(),
            prev_object_to_world: vec![Mat4::IDENTITY; MAX_DYNAMIC_INSTANCES],
            static_material_indices: Vec::new(),
            dynamic_material_indices: Vec::new(),
            frame_idx: 0,
            bind_group_layout,
        }
    }

    pub fn write_vertex_data(
        &self,
        data: &VertexPoolWriteData,
        alloc: &VertexPoolAlloc,
        queue: &wgpu::Queue,
    ) {
        let slice = &self.slices[alloc.index as usize];

        queue.write_buffer(
            &self.vertex_buffer,
            (slice.first_vertex as usize * std::mem::size_of::<PackedVertex>()) as u64,
            bytemuck::cast_slice(data.packed_vertices),
        );

        queue.write_buffer(
            &self.index_buffer,
            (slice.first_index as usize * std::mem::size_of::<u32>()) as u64,
            bytemuck::cast_slice(data.indices),
        );

        queue.write_buffer(
            &self.triangle_material_index_buffer,
            (slice.first_index as usize / 3 * std::mem::size_of::<u32>()) as u64,
            bytemuck::cast_slice(data.triangle_material_indices),
        );
    }

    pub fn write_slices(&mut self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.slices_buffer, 0, bytemuck::cast_slice(&self.slices));

        queue.write_buffer(
            &self.object_to_world_buffer,
            0,
            bytemuck::cast_slice(&self.delta_object_to_world_inv),
        );

        queue.write_buffer(
            &self.material_index_buffer,
            0,
            bytemuck::cast_slice(&self.dynamic_material_indices),
        );
        if !self.static_material_indices.is_empty() {
            queue.write_buffer(
                &self.material_index_buffer,
                (size_of::<u32>() * MAX_MATERIALS_PER_INSTANCE * MAX_DYNAMIC_INSTANCES) as u64,
                bytemuck::cast_slice(&self.static_material_indices),
            );
        }
    }

    pub fn submit_slice_instance(
        &mut self,
        transform: Mat4,
        is_static: bool,
        materials: &[Arc<GpuMaterial>],
    ) {
        assert!(materials.len() < MAX_MATERIALS_PER_INSTANCE);

        if is_static {
            for material in materials {
                self.static_material_indices.push(material.material_idx);
            }
            for _ in 0..(MAX_MATERIALS_PER_INSTANCE - materials.len()) {
                self.static_material_indices.push(0);
            }
        } else {
            let i = self.delta_object_to_world_inv.len();
            let delta = transform * self.prev_object_to_world[i].inverse();
            self.delta_object_to_world_inv.push(delta.inverse());
            self.prev_object_to_world[i] = transform;
            assert!(self.delta_object_to_world_inv.len() < MAX_DYNAMIC_INSTANCES);

            for material in materials {
                self.dynamic_material_indices.push(material.material_idx);
            }
            for _ in 0..(MAX_MATERIALS_PER_INSTANCE - materials.len()) {
                self.dynamic_material_indices.push(0);
            }
        }
    }

    pub fn end_frame(&mut self) {
        self.delta_object_to_world_inv.clear();
        self.static_material_indices.clear();
        self.dynamic_material_indices.clear();

        self.frame_idx += 1;
    }

    pub fn alloc(&mut self, num_vertices: u32, num_indices: u32) -> VertexPoolAlloc {
        let slice_idx = self
            .first_available_slice_idx()
            .expect("Vertex pool ran out of slices!");

        let vertex_alloc = self
            .vertex_allocator
            .allocate(num_vertices as u64)
            .expect("Failed to allocate vertices.");
        let index_alloc = self
            .index_allocator
            .allocate(num_indices as u64)
            .expect("Failed to allocate indices.");

        let slice = VertexPoolSlice::new(
            vertex_alloc.start() as u32,
            num_vertices,
            index_alloc.start() as u32,
            num_indices,
        );
        self.slices[slice_idx] = slice;

        VertexPoolAlloc {
            vertex_alloc,
            index_alloc,
            index: slice_idx as u32,
        }
    }

    pub fn free(&mut self, alloc: &VertexPoolAlloc) {
        self.vertex_allocator.free(&alloc.vertex_alloc);
        self.index_allocator.free(&alloc.index_alloc);
        self.slices[alloc.index as usize].num_vertices = 0;
    }

    fn first_available_slice_idx(&self) -> Option<usize> {
        for (i, slice) in self.slices.iter().enumerate() {
            if !slice.is_allocated() {
                return Some(i);
            }
        }

        None
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.triangle_material_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.slices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.object_to_world_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.material_index_buffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }
}
