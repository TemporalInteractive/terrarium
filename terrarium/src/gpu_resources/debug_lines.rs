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

        Self {
            vertex_buffer,
            vertices: Vec::new(),
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
        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(self.vertices.as_slice()),
        );
    }

    pub fn end_frame(&mut self) {
        self.vertices.clear();
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }
}
