mod pass;
mod pipeline;
mod render_resource_manager;

pub use pass::Pass;
pub use pipeline::Pipeline;
pub use render_resource_manager::RenderResourceManager;

use crate::render::UniformBuffer;
use std::collections::HashMap;
use legion::world::World;

pub struct RenderGraph {
    pub data: RenderGraphData,
    passes: HashMap<String, Box<dyn Pass>>,
    pipelines: HashMap<String, Box<dyn Pipeline>>,
    pass_pipelines: HashMap<String, Vec<String>>,
    render_resource_managers: Vec<Box<dyn RenderResourceManager>>,
    pub swap_chain: wgpu::SwapChain, // TODO: this is weird
}

pub struct RenderGraphData {
    pub swap_chain_descriptor: wgpu::SwapChainDescriptor,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    textures: HashMap<String, wgpu::TextureView>,
    samplers: HashMap<String, wgpu::Sampler>,
    uniform_buffers: HashMap<String, UniformBuffer>,
    bind_group_layouts: HashMap<String, wgpu::BindGroupLayout>,
}
impl RenderGraphData {
    pub fn new(device: wgpu::Device, swap_chain_descriptor: wgpu::SwapChainDescriptor, queue: wgpu::Queue, surface: wgpu::Surface) -> Self {
        RenderGraphData {
            textures: HashMap::new(),
            samplers: HashMap::new(),
            uniform_buffers: HashMap::new(),
            bind_group_layouts: HashMap::new(),
            device,
            swap_chain_descriptor,
            queue,
            surface,
        }
    }

    pub fn set_uniform_buffer(&mut self, name: &str, uniform_buffer: UniformBuffer) {
        self.uniform_buffers.insert(name.to_string(), uniform_buffer);
    }

    pub fn get_uniform_buffer(&self, name: &str) -> Option<&UniformBuffer> {
        self.uniform_buffers.get(name)
    }

    pub fn set_bind_group_layout(&mut self, name: &str, bind_group_layout: wgpu::BindGroupLayout) {
        self.bind_group_layouts.insert(name.to_string(), bind_group_layout);
    }

    pub fn get_bind_group_layout(&self, name: &str) -> Option<&wgpu::BindGroupLayout> {
        self.bind_group_layouts.get(name)
    }

    pub fn set_texture(&mut self, name: &str, texture: wgpu::TextureView) {
        self.textures.insert(name.to_string(), texture);
    }

    pub fn get_texture(&self, name: &str) -> Option<&wgpu::TextureView> {
        self.textures.get(name)
    }

    pub fn set_sampler(&mut self, name: &str, sampler: wgpu::Sampler) {
        self.samplers.insert(name.to_string(), sampler);
    }

    pub fn get_sampler(&self, name: &str) -> Option<&wgpu::Sampler> {
        self.samplers.get(name)
    }
}

impl RenderGraph {
    pub fn new(device: wgpu::Device, swap_chain_descriptor: wgpu::SwapChainDescriptor, swap_chain: wgpu::SwapChain, queue: wgpu::Queue, surface: wgpu::Surface) -> Self {
        RenderGraph {
            passes: HashMap::new(),
            pipelines: HashMap::new(),
            swap_chain,
            pass_pipelines: HashMap::new(),
            render_resource_managers: Vec::new(),
            data: RenderGraphData::new(device, swap_chain_descriptor, queue, surface),
        }
    }

    pub fn initialize(&mut self, world: &mut World) {
        for render_resource_manager in self.render_resource_managers.iter_mut() {
            render_resource_manager.initialize(&mut self.data, world);
        }

        for pass in self.passes.values_mut() {
            pass.initialize(&mut self.data);
        }
        
        for pipeline in self.pipelines.values_mut() {
            pipeline.initialize(&mut self.data, world);
        }
    }

    pub fn render(&mut self, world: &mut World) {
        let frame = self.swap_chain
            .get_next_texture()
            .expect("Timeout when acquiring next swap chain texture");

        let mut encoder =
            self.data.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        for render_resource_manager in self.render_resource_managers.iter_mut() {
            render_resource_manager.update(&mut self.data, &mut encoder, world);
        }

        for (pass_name, pass) in self.passes.iter_mut() {
            loop {
                let render_pass = pass.begin(&mut self.data, world, &mut encoder, &frame);
                if let Some(mut render_pass) = render_pass {
                    // TODO: assign pipelines to specific passes
                    if let Some(pipeline_names)  = self.pass_pipelines.get(pass_name) {
                        for pipeline_name in pipeline_names.iter() {
                            let pipeline = self.pipelines.get_mut(pipeline_name).unwrap();
                            render_pass.set_pipeline(pipeline.get_pipeline());
                            pipeline.render(&mut self.data, &mut render_pass, &frame, world);
                        }
                    }

                    if !pass.should_repeat() {
                        break;
                    }
                }
            }
        }

        let command_buffer = encoder.finish();
        self.data.queue.submit(&[command_buffer]);
    }

    pub fn resize(&mut self, width: u32, height: u32, world: &mut World) {
        self.data.swap_chain_descriptor.width = width;
        self.data.swap_chain_descriptor.height = height;
        self.swap_chain = self.data.device.create_swap_chain(&self.data.surface, &self.data.swap_chain_descriptor);
        let mut encoder =
            self.data.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        for render_resource_manager in self.render_resource_managers.iter_mut() {
            render_resource_manager.resize(&mut self.data, &mut encoder, world);
        }

        let command_buffer = encoder.finish();

        for pass in self.passes.values_mut() {
            pass.resize(&mut self.data);
        }

        for pipeline in self.pipelines.values_mut() {
            pipeline.resize(&mut self.data);
        }

        self.data.queue.submit(&[command_buffer]);

    }

    pub fn add_render_resource_manager(&mut self, render_resource_manager: Box<dyn RenderResourceManager>) {
        self.render_resource_managers.push(render_resource_manager);
    }

    pub fn set_pipeline(&mut self, pass_name: &str, pipeline_name: &str, pipeline: Box<dyn Pipeline>) {
        self.pipelines.insert(pipeline_name.to_string(), pipeline);
        if let None = self.pass_pipelines.get_mut(pass_name) {
            let pipelines = Vec::new();
            self.pass_pipelines.insert(pass_name.to_string(), pipelines);
        };

        let current_pass_pipelines = self.pass_pipelines.get_mut(pass_name).unwrap();

        current_pass_pipelines.push(pipeline_name.to_string());
    }

    pub fn set_pass(&mut self, name: &str, pass: Box<dyn Pass>) {
        self.passes.insert(name.to_string(), pass);
    }
}