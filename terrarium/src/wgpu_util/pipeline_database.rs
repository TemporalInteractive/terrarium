use core::str;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

pub struct PipelineDatabase {
    shader_modules: HashMap<String, Arc<wgpu::ShaderModule>>,
    render_pipelines: HashMap<String, Arc<wgpu::RenderPipeline>>,
    compute_pipelines: HashMap<String, Arc<wgpu::ComputePipeline>>,
}

impl Default for PipelineDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineDatabase {
    pub fn new() -> Self {
        Self {
            shader_modules: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
        }
    }

    pub fn shader_from_src(&mut self, device: &wgpu::Device, src: &str) -> Arc<wgpu::ShaderModule> {
        if let Some(module) = self.shader_modules.get(src) {
            return module.clone();
        }

        let module = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(src),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(src)),
        }));

        self.shader_modules.insert(src.to_owned(), module.clone());
        module
    }

    pub fn render_pipeline<F>(
        &mut self,
        device: &wgpu::Device,
        descriptor: wgpu::RenderPipelineDescriptor,
        create_layout_fn: F,
    ) -> Arc<wgpu::RenderPipeline>
    where
        F: Fn() -> wgpu::PipelineLayout,
    {
        let entry = descriptor
            .label
            .expect("Every pipeline must contain a label!");
        if let Some(pipeline) = self.render_pipelines.get(entry) {
            return pipeline.clone();
        }

        let pipeline_layout = create_layout_fn();
        let descriptor = wgpu::RenderPipelineDescriptor {
            layout: Some(&pipeline_layout),
            ..descriptor
        };

        let pipeline = Arc::new(device.create_render_pipeline(&descriptor));

        self.render_pipelines
            .insert(entry.to_owned(), pipeline.clone());
        pipeline
    }

    pub fn compute_pipeline<F>(
        &mut self,
        device: &wgpu::Device,
        descriptor: wgpu::ComputePipelineDescriptor,
        create_layout_fn: F,
    ) -> Arc<wgpu::ComputePipeline>
    where
        F: Fn() -> wgpu::PipelineLayout,
    {
        let entry = descriptor
            .label
            .expect("Every pipeline must contain a label!");
        if let Some(pipeline) = self.compute_pipelines.get(entry) {
            return pipeline.clone();
        }

        let pipeline_layout = create_layout_fn();
        let descriptor = wgpu::ComputePipelineDescriptor {
            layout: Some(&pipeline_layout),
            ..descriptor
        };

        let pipeline = Arc::new(device.create_compute_pipeline(&descriptor));

        self.compute_pipelines
            .insert(entry.to_owned(), pipeline.clone());
        pipeline
    }
}
