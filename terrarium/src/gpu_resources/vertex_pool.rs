use bytemuck::{Pod, Zeroable};
use ugm::mesh::PackedVertex;

use super::linear_block_allocator::LinearBlockAllocator;

pub const MAX_VERTEX_POOL_VERTICES: usize = 1024 * 1024 * 32;
pub const MAX_VERTEX_POOL_SLICES: usize = 1024 * 16;

pub struct VertexPoolWriteData<'a> {
    pub packed_vertices: &'a [PackedVertex],
    pub indices: &'a [u32],
    pub triangle_material_indices: &'a [u32],
}

#[derive(Debug, Clone)]
pub struct VertexPoolAlloc {
    pub slice: VertexPoolSlice,
    pub index: u32,
}

#[derive(Pod, Debug, Clone, Copy, Zeroable, PartialEq, Eq)]
#[repr(C)]
pub struct VertexPoolSlice {
    first_vertex: u32,
    num_vertices: u32,
    first_index: u32,
    num_indices: u32,
    pub material_idx: u32,
    is_allocated_and_padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

impl VertexPoolSlice {
    fn new(
        first_vertex: u32,
        num_vertices: u32,
        first_index: u32,
        num_indices: u32,
        material_idx: u32,
    ) -> Self {
        Self {
            first_vertex,
            num_vertices,
            first_index,
            num_indices,
            material_idx,
            is_allocated_and_padding0: 0,
            _padding1: 0,
            _padding2: 0,
        }
    }

    fn new_unallocated() -> Self {
        Self {
            first_vertex: 0,
            num_vertices: 0,
            first_index: 0,
            num_indices: 0,
            material_idx: 0,
            is_allocated_and_padding0: u32::MAX,
            _padding1: 0,
            _padding2: 0,
        }
    }

    fn is_allocated(&self) -> bool {
        self.is_allocated_and_padding0 != u32::MAX
    }

    pub fn first_vertex(&self) -> u32 {
        self.first_vertex
    }

    pub fn first_index(&self) -> u32 {
        self.first_index
    }

    fn last_vertex(&self) -> u32 {
        self.first_vertex + self.num_vertices
    }

    pub fn last_index(&self) -> u32 {
        self.first_index + self.num_indices
    }
}

pub struct VertexPool {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    triangle_material_index_buffer: wgpu::Buffer,
    slices_buffer: wgpu::Buffer,

    vertex_allocator: LinearBlockAllocator,
    index_allocator: LinearBlockAllocator,
    slices: Box<[VertexPoolSlice]>,

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
            size: (std::mem::size_of::<u32>() * MAX_VERTEX_POOL_VERTICES * 3) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT
                | wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let triangle_material_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool triangle_material_indices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<u32>() * MAX_VERTEX_POOL_VERTICES / 3) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let slices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::vertex_pool slices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<VertexPoolSlice>() * MAX_VERTEX_POOL_SLICES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let vertex_allocator = LinearBlockAllocator::new(MAX_VERTEX_POOL_VERTICES as u64);
        let index_allocator = LinearBlockAllocator::new(MAX_VERTEX_POOL_VERTICES as u64 / 3);

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
            ],
        });

        Self {
            vertex_buffer,
            index_buffer,
            triangle_material_index_buffer,
            slices_buffer,

            vertex_allocator,
            index_allocator,
            slices: vec![VertexPoolSlice::new_unallocated(); MAX_VERTEX_POOL_SLICES]
                .into_boxed_slice(),
            bind_group_layout,
        }
    }

    pub fn write_vertex_data(
        &self,
        data: &VertexPoolWriteData,
        slice: VertexPoolSlice,
        queue: &wgpu::Queue,
    ) {
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
    }

    pub fn alloc(
        &mut self,
        num_vertices: u32,
        num_indices: u32,
        material_idx: u32,
    ) -> VertexPoolAlloc {
        let slice_idx = self
            .first_available_slice_idx()
            .expect("Vertex pool ran out of slices!");
        let first_vertex = self
            .first_available_vertex(num_vertices)
            .expect("Vertex pool ran out of vertices!");
        let first_index = self
            .first_available_index(num_indices)
            .expect("Vertex pool ran out of indices!");

        let slice = VertexPoolSlice::new(
            first_vertex,
            num_vertices,
            first_index,
            num_indices,
            material_idx,
        );
        self.slices[slice_idx] = slice;

        VertexPoolAlloc {
            slice,
            index: slice_idx as u32,
        }
    }

    pub fn free(index: u32) {}

    fn first_available_slice_idx(&self) -> Option<usize> {
        for (i, slice) in self.slices.iter().enumerate() {
            if !slice.is_allocated() {
                return Some(i);
            }
        }

        None
    }

    fn first_available_vertex(&self, num_vertices: u32) -> Option<u32> {
        if self.slices.is_empty() && MAX_VERTEX_POOL_VERTICES as u32 > num_vertices {
            return Some(0);
        }

        for i in 0..self.slices.len() {
            if self.slices[i].is_allocated() {
                continue;
            }

            let prev = if i > 0 {
                self.slices[i - 1].last_vertex()
            } else {
                0
            };

            let space = self.slices[i].first_vertex - prev;
            if space >= num_vertices {
                return Some(prev + num_vertices);
            }
        }

        let back = self.slices.last().unwrap().last_vertex();
        if back + num_vertices <= MAX_VERTEX_POOL_VERTICES as u32 {
            return Some(back);
        }

        None
    }

    fn first_available_index(&self, num_indices: u32) -> Option<u32> {
        if self.slices.is_empty() && MAX_VERTEX_POOL_VERTICES as u32 * 3 > num_indices {
            return Some(0);
        }

        for i in 0..self.slices.len() {
            if self.slices[i].is_allocated() {
                continue;
            }

            let prev = if i > 0 {
                self.slices[i - 1].last_index()
            } else {
                0
            };

            let space = self.slices[i].first_index - prev;
            if space >= num_indices {
                return Some(prev + num_indices);
            }
        }

        let back = self.slices.last().unwrap().last_index();
        if back + num_indices <= MAX_VERTEX_POOL_VERTICES as u32 {
            return Some(back);
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
