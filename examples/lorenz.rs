use std::sync::Arc;

use eframe::egui::plot::{Legend, PlotImage};
use eframe::egui::{self, plot::PlotBounds};
use eframe::emath::Vec2;
use wgpu;

use egui_gpu_plot::*;

const MAX_POINTS: usize = 1_000_000;

pub struct GpuPlot {
    q: [f32; 3],

    show_cpu: bool,
    show_gpu: bool,

    dirty: bool,
    texture_id: egui::TextureId,
    points: Arc<Vec<Vertex>>,
}

impl GpuPlot {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Option<Self> {
        let wgpu_render_state = cc.wgpu_render_state.as_ref()?;

        let device = &wgpu_render_state.device;
        let target_format = wgpu_render_state.target_format;

        let plot = GpuAcceleratedPlot::new(device, target_format);
        let texture_id = {
            let mut renderer = wgpu_render_state.renderer.write();
            renderer.register_native_texture(device, &plot.create_view(), wgpu::FilterMode::Linear)
        };

        wgpu_render_state
            .renderer
            .write()
            .paint_callback_resources
            .insert(plot);

        let q = [10.0, 28.0, 8.0 / 3.0];
        Some(Self {
            q,
            show_cpu: false,
            show_gpu: true,
            dirty: true,
            texture_id,
            points: Arc::new(forward_euler(lorenz, q, MAX_POINTS)),
        })
    }
}

fn lorenz(q: [f32; 3], s: [f32; 3]) -> [f32; 3] {
    let sigma = q[0];
    let rho = q[1];
    let beta = q[2];

    [
        sigma * (s[1] - s[0]),
        s[0] * (rho - s[2]) - s[1],
        s[0] * s[1] - beta * s[2],
    ]
}

fn forward_euler<F>(df: F, q: [f32; 3], n: usize) -> Vec<Vertex>
where
    F: Fn([f32; 3], [f32; 3]) -> [f32; 3],
{
    let tf = 1000.0;
    let dt = tf / n as f32;

    let mut s = [1.0, 0.0, 0.0];
    let mut vs = Vec::with_capacity(n);

    for i in 0..n {
        let pct = i as f32 / n as f32;

        let ds = df(q, s);
        for j in 0..s.len() {
            s[j] += ds[j] * dt;
        }

        let position = [s[0], s[2]];
        let normal = egui::Vec2::new(ds[0], ds[2]).normalized().rot90();
        let color = egui::color::Hsva::new(pct, 0.85, 0.5, 1.0).to_rgba_premultiplied();

        vs.push(Vertex {
            position,
            normal: [normal.x, normal.y],
            color,
        });
        // two vertices per
        vs.push(Vertex {
            position,
            normal: [-normal.x, -normal.y],
            color,
        });
    }

    vs
}

impl eframe::App for GpuPlot {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut new_sigma = self.q[0];
            let mut new_rho = self.q[1];
            let mut new_beta = self.q[2];

            ui.horizontal(|ui| {
                for (l, v, range) in [
                    ("σ", &mut new_sigma, 0.0..=20.0),
                    ("ρ", &mut new_rho, 0.0..=50.0),
                    ("β", &mut new_beta, 0.0..=10.0),
                ] {
                    ui.label(l);
                    ui.add(egui::Slider::new(v, range).step_by(0.01));
                }

                ui.toggle_value(&mut self.show_cpu, "CPU");
                ui.toggle_value(&mut self.show_gpu, "GPU");
            });

            if self.q != [new_sigma, new_rho, new_beta] {
                self.q = [new_sigma, new_rho, new_beta];

                self.points = Arc::new(forward_euler(lorenz, self.q, MAX_POINTS));
                self.dirty = true;
            }

            let mut bounds = PlotBounds::NOTHING;
            let resp = egui::plot::Plot::new("my_plot")
                .legend(Legend::default())
                // Must set margins to zero or the image and plot bounds will
                // constantly fight, expanding the plot to infinity.
                .set_margin_fraction(Vec2::new(0.0, 0.0))
                .include_x(-25.0)
                .include_x(25.0)
                .include_y(0.0)
                .include_y(60.0)
                .show(ui, |ui| {
                    bounds = ui.plot_bounds();

                    if self.show_gpu {
                        // Render the plot texture filling the viewport.
                        ui.image(
                            PlotImage::new(
                                self.texture_id,
                                bounds.center(),
                                [bounds.width() as f32, bounds.height() as f32],
                            )
                            .name("Lorenz attractor (GPU)"),
                        );
                    }

                    if self.show_cpu {
                        ui.line(
                            egui::plot::Line::new(egui::plot::PlotPoints::from_iter(
                                self.points
                                    .iter()
                                    .map(|p| [p.position[0] as f64, p.position[1] as f64]),
                            ))
                            .name("Lorenz attractor (CPU)"),
                        );
                    }
                });

            if self.show_gpu {
                // Add a callback to egui to render the plot contents to
                // texture.
                ui.painter().add(egui_wgpu_callback(
                    bounds,
                    Arc::clone(&self.points),
                    resp.response.rect,
                    self.dirty,
                ));

                // Update the texture handle in egui from the previously
                // rendered texture (from the last frame).
                let wgpu_render_state = frame.wgpu_render_state().unwrap();
                let mut renderer = wgpu_render_state.renderer.write();

                let plot: &GpuAcceleratedPlot = renderer.paint_callback_resources.get().unwrap();
                let texture_view = plot.create_view();

                renderer.update_egui_texture_from_wgpu_texture(
                    &wgpu_render_state.device,
                    &texture_view,
                    wgpu::FilterMode::Linear,
                    self.texture_id,
                );

                self.dirty = false;
            }
        });
    }
}

fn main() {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "GPU Accelerated Plotter",
        native_options,
        Box::new(|cc| Box::new(GpuPlot::new(cc).unwrap())),
    );
}
