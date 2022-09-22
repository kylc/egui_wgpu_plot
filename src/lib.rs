use std::{iter, sync::Arc};

use egui::plot::PlotBounds;
use wgpu::{util::DeviceExt, TextureViewDescriptor};

const MSAA_SAMPLE_COUNT: u32 = 1;
const MAX_POINTS: usize = 5_000_000;

const DEFAULT_WIDTH: u32 = 1;
const DEFAULT_HEIGHT: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub normal: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniform {
    pub x_bounds: [f32; 2],
    pub y_bounds: [f32; 2],
}

pub struct GpuAcceleratedPlot {
    pipeline: wgpu::RenderPipeline,
    target_format: wgpu::TextureFormat,
    bind_group: wgpu::BindGroup,

    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,

    texture: (wgpu::Texture, wgpu::TextureView),
    multisampled_texture: (wgpu::Texture, wgpu::TextureView),
    width: u32,
    height: u32,
}

impl GpuAcceleratedPlot {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> GpuAcceleratedPlot {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("egui_plot_line_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./line_shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("egui_plot_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("egui_plot_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui_plot_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLE_COUNT,
                ..Default::default()
            },
            multiview: None,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("egui_plot_uniforms"),
            contents: bytemuck::cast_slice(&[Uniform {
                x_bounds: [-1.0, 1.0],
                y_bounds: [-1.0, 1.0],
            }]),
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::MAP_WRITE
                | wgpu::BufferUsages::UNIFORM,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("egui_plot_vertices"),
            contents: bytemuck::cast_slice(&vec![Vertex::default(); MAX_POINTS]),
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::MAP_WRITE
                | wgpu::BufferUsages::VERTEX,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui_plot_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Allocate some stand-in textures since we don't know the final width
        // and height yet.
        let texture = Self::create_texture(device, target_format, 1, DEFAULT_WIDTH, DEFAULT_HEIGHT);
        let multisampled_texture = Self::create_texture(
            device,
            target_format,
            MSAA_SAMPLE_COUNT,
            DEFAULT_WIDTH,
            DEFAULT_HEIGHT,
        );

        GpuAcceleratedPlot {
            pipeline,
            target_format,
            bind_group,
            uniform_buffer,
            vertex_buffer,
            vertex_count: 0,
            texture,
            multisampled_texture,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
        }
    }

    fn create_texture(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        sample_count: u32,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("egui_plot_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: target_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });

        let view = texture.create_view(&TextureViewDescriptor::default());

        (texture, view)
    }

    pub fn create_view(&self) -> wgpu::TextureView {
        self.texture
            .0
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_multisampled_view(&self) -> wgpu::TextureView {
        self.multisampled_texture
            .0
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        dimensions: [u32; 2],
        bounds: &PlotBounds,
        points: &[Vertex],
        dirty: bool,
    ) {
        // Re-allocate the render targets if the requested dimensions have changed.
        if dimensions[0] != self.width || dimensions[1] != self.height {
            self.width = dimensions[0];
            self.height = dimensions[1];

            self.texture =
                Self::create_texture(device, self.target_format, 1, self.width, self.height);
            self.multisampled_texture = Self::create_texture(
                device,
                self.target_format,
                MSAA_SAMPLE_COUNT,
                self.width,
                self.height,
            );
        }

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniform {
                x_bounds: [bounds.min()[0] as f32, bounds.max()[0] as f32],
                y_bounds: [bounds.min()[1] as f32, bounds.max()[1] as f32],
            }]),
        );

        // Only re-upload the vertex buffer if it has changed.
        // TODO: for time-series charts where the buffer acts as a ring, we
        // could be smart about updating only the subset of added/removed
        // vertices.
        if dirty {
            self.vertex_count = points.len() as u32;
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(points));
        }
    }

    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let view = self.create_view();
            let msaa_view = self.create_multisampled_view();

            // Render directly to the texture if no MSAA, or use the
            // multisampled buffer and resolve to the texture if using MSAA.
            let rpass_color_attachment = if MSAA_SAMPLE_COUNT == 1 {
                wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }
            } else {
                wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: false,
                    },
                }
            };

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(rpass_color_attachment)],
                depth_stencil_attachment: None,
            });

            self.render_onto_renderpass(&mut rpass);
        }

        queue.submit(iter::once(encoder.finish()));
    }

    pub fn render_onto_renderpass<'rp>(&'rp self, rpass: &mut wgpu::RenderPass<'rp>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..self.vertex_count, 0..1);
    }
}

pub fn egui_wgpu_callback(
    bounds: PlotBounds,
    points: Arc<Vec<Vertex>>,
    rect: egui::Rect,
    dirty: bool,
) -> egui::PaintCallback {
    let cb =
        egui_wgpu::CallbackFn::new().prepare(move |device, queue, paint_callback_resources| {
            let plot: &mut GpuAcceleratedPlot = paint_callback_resources.get_mut().unwrap();

            plot.prepare(
                device,
                queue,
                [rect.width() as u32, rect.height() as u32],
                &bounds,
                &points,
                dirty,
            );

            plot.render(device, queue);
        });

    egui::PaintCallback {
        rect,
        callback: Arc::new(cb),
    }
}
