use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use wgpu::util::DeviceExt;

#[derive(Debug, Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct SunInfo {
    pub direction: Vec3,
    pub size: f32,
    pub color: Vec3,
    pub intensity: f32,
}

impl SunInfo {
    #[cfg(feature = "egui")]
    pub fn egui(&mut self, ui: &mut egui::Ui) {
        let mut direction = self.direction.to_array();
        ui.horizontal(|ui| {
            ui.label("Direction");
            ui.add(
                egui::DragValue::new(&mut direction[0])
                    .speed(0.01)
                    .range(-1.0..=1.0)
                    .prefix("x: "),
            );
            ui.add(
                egui::DragValue::new(&mut direction[1])
                    .speed(0.01)
                    .range(-1.0..=1.0)
                    .prefix("y: "),
            );
            ui.add(
                egui::DragValue::new(&mut direction[2])
                    .speed(0.01)
                    .range(-1.0..=1.0)
                    .prefix("z: "),
            );
        });
        self.direction = Vec3::from_array(direction).normalize_or_zero();

        ui.add(egui::Slider::new(&mut self.size, 0.0..=1.0).text("Size"));

        let mut color = self.color.to_array();
        ui.color_edit_button_rgb(&mut color)
            .labelled_by(ui.label("Color").id);
        self.color = Vec3::from_array(color);

        ui.add(egui::Slider::new(&mut self.intensity, 0.0..=500.0).text("Intensity"));
    }
}

impl Default for SunInfo {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.2, -1.0, 0.3).normalize(),
            color: Vec3::new(1.0, 1.0, 1.0),
            size: 0.0,
            intensity: 3.0,
        }
    }
}

#[derive(Debug, Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct AtmosphereInfo {
    pub inscattering_color: Vec3,
    pub density: f32,
}

impl Default for AtmosphereInfo {
    fn default() -> Self {
        Self {
            inscattering_color: Vec3::new(135.0 / 255.0, 206.0 / 255.0, 235.0 / 255.0).normalize(),
            density: 0.005,
        }
    }
}

impl AtmosphereInfo {
    #[cfg(feature = "egui")]
    pub fn egui(&mut self, ui: &mut egui::Ui) {
        let mut inscattering_color = self.inscattering_color.to_array();
        ui.color_edit_button_rgb(&mut inscattering_color)
            .labelled_by(ui.label("Inscattering Color").id);
        self.inscattering_color = Vec3::from_array(inscattering_color);

        ui.add(egui::Slider::new(&mut self.density, 0.0..=0.3).text("Density"));
    }
}

#[derive(Debug, Default, Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct SkyConstants {
    pub sun: SunInfo,
    pub atmosphere: AtmosphereInfo,
    pub world_up: Vec3,
    _padding0: u32,
}

pub struct Sky {
    bind_group_layout: wgpu::BindGroupLayout,
    pub constants: SkyConstants,
}

impl Sky {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        Self {
            bind_group_layout,
            constants: Default::default(),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrarium::sky constants"),
            contents: bytemuck::bytes_of(&self.constants),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: constants.as_entire_binding(),
            }],
        })
    }
}
