use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};

const MAX_LINES: u64 = 1024 * 1024 * 16;

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Vertex {
    position: Vec4,
    color: Vec4, // TODO: pack
}

pub struct DebugLines {
    vertex_buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,

    gpu_vertex_buffer: wgpu::Buffer,
    gpu_vertex_count_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl DebugLines {
    pub const VERTEX_BUFFER_LAYOUT: wgpu::VertexBufferLayout<'_> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 4 * 4,
                shader_location: 1,
            },
        ],
    };

    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::debug_lines vertices"),
            size: std::mem::size_of::<Vertex>() as u64 * MAX_LINES * 2,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let gpu_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::debug_lines gpu_vertices"),
            size: std::mem::size_of::<Vertex>() as u64 * MAX_LINES * 2,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let gpu_vertex_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrarium::debug_lines gpu_vertex_counter"),
            size: std::mem::size_of::<u32>() as u64 * 4,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::INDIRECT,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu_vertex_count_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_vertex_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            vertex_buffer,
            vertices: Vec::new(),
            gpu_vertex_buffer,
            gpu_vertex_count_buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn submit_line(&mut self, start: Vec3, end: Vec3, color: Vec3) {
        self.vertices.push(Vertex {
            position: Vec4::from((start, 1.0)),
            color: Vec4::from((color, 1.0)),
        });
        self.vertices.push(Vertex {
            position: Vec4::from((end, 1.0)),
            color: Vec4::from((color, 1.0)),
        });
    }

    pub fn write_lines(&mut self, queue: &wgpu::Queue) {
        if !self.vertices.is_empty() {
            queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(self.vertices.as_slice()),
            );
        }
    }

    pub fn end_frame(&mut self, command_encoder: &mut wgpu::CommandEncoder) {
        self.vertices.clear();

        command_encoder.clear_buffer(&self.gpu_vertex_count_buffer, 0, None);
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }

    pub fn gpu_vertex_buffer(&self) -> &wgpu::Buffer {
        &self.gpu_vertex_buffer
    }

    pub fn gpu_vertex_count_buffer(&self) -> &wgpu::Buffer {
        &self.gpu_vertex_count_buffer
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
