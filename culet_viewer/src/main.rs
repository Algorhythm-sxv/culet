use std::sync::{mpsc::Receiver, Arc};

use culet_lib::{
    camera::Camera,
    glam::{vec3, Mat3, Vec3},
    mesh::Mesh,
    render::{AbortSignal, RenderMsg, RenderOptions},
    scene::Scene,
};
use eframe::{run_native, App, CreationContext, NativeOptions};
use egui::{
    load::SizedTexture, CentralPanel, Color32, ColorImage, DragValue, ImageSource, Sense,
    TextureHandle, TextureOptions, Vec2,
};

const DEFAULT_SIZE: usize = 800;

struct CuletViewer {
    frame_buffer: ColorImage,
    render_buffer_handle: TextureHandle,
    render_options: RenderOptions,
    render_stream: Receiver<RenderMsg>,
    render_abort: AbortSignal,
}

impl CuletViewer {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let render_buffer_handle = cc.egui_ctx.load_texture(
            "Render output",
            ColorImage::new([DEFAULT_SIZE, DEFAULT_SIZE], Color32::BLACK),
            TextureOptions::LINEAR,
        );
        let camera = Camera::default()
            .fov(12.0)
            .position(vec3(0.2, 0.0, 10.0))
            .look_at(vec3(0.0, 0.0, -1.5))
            .aspect_ratio(1.0);

        let scene = Scene::new(vec![Mesh::load_from_stl(
            vec3(0.0, 0.0, -1.5),
            "../lowboy.stl",
        )]);
        let render_options = RenderOptions::new()
            .camera(camera)
            .scene(Arc::new(scene))
            .threads(12)
            .background_color(Vec3::splat(0.0))
            .samples_per_pixel(1)
            .max_bounces(8)
            .image_width(DEFAULT_SIZE)
            .image_height(DEFAULT_SIZE);

        let (render_stream, render_abort) = render_options.render_streaming();

        Self {
            frame_buffer: ColorImage::new([DEFAULT_SIZE, DEFAULT_SIZE], Color32::BLACK),
            render_buffer_handle,
            render_options,
            render_stream,
            render_abort,
        }
    }
}

impl App for CuletViewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_buffer_handle
            .set(self.frame_buffer.clone(), TextureOptions::LINEAR);

        let mut render_dirty = false;
        let old_settings = (
            self.render_options.image_width,
            self.render_options.max_bounces,
        );
        CentralPanel::default().show(ctx, |ui| {
            ui.label("Culet Viewer");
            ui.vertical_centered(|ui| {
                let image = egui::Image::new(ImageSource::Texture(SizedTexture::from_handle(
                    &self.render_buffer_handle,
                )))
                .fit_to_exact_size(Vec2::splat(DEFAULT_SIZE as f32))
                .sense(Sense::drag());

                // apply rotations
                let response = ui.add(image).drag_delta();
                if response != Vec2::splat(0.0) {
                    let rotation_x = Mat3::from_rotation_x(-response[1] * 0.001);
                    let rotation_y = Mat3::from_rotation_y(-response[0] * 0.001);
                    self.render_options.camera = self
                        .render_options
                        .camera
                        .position(rotation_x * rotation_y * self.render_options.camera.position)
                        .look_at(vec3(0.0, 0.0, -1.5));
                    render_dirty = true;
                }
            });

            // settings panel
            ui.horizontal_centered(|ui| {
                // render size
                ui.vertical(|ui| {
                    ui.label("Image Size:");
                    ui.add(
                        DragValue::new(&mut self.render_options.image_width)
                            .clamp_range(0..=800)
                            .speed(10),
                    )
                });

                // max bounces
                ui.vertical(|ui| {
                    ui.label("Max Bounces");
                    ui.add(
                        DragValue::new(&mut self.render_options.max_bounces)
                            .clamp_range(1..=20)
                            .speed(0.1),
                    );
                });
            });
        });
        ctx.request_repaint();

        let resolution_changed = self.render_options.image_width != old_settings.0;
        if (
            self.render_options.image_width,
            self.render_options.max_bounces,
        ) != old_settings
        {
            self.render_options.image_height = self.render_options.image_width;
            render_dirty = true;
        }

        if render_dirty {
            // abort previous render
            self.render_abort.abort();

            // clear frame buffer
            if resolution_changed {
                self.frame_buffer = ColorImage::new(
                    [
                        self.render_options.image_width,
                        self.render_options.image_height,
                    ],
                    Color32::BLACK,
                );
            }

            // start new render
            let (stream, abort) = self.render_options.render_streaming();
            self.render_stream = stream;
            self.render_abort = abort;
        }

        // update max 10000px per frame
        for _ in 0..100000 {
            let Ok(px_msg) = self.render_stream.try_recv() else {
                break;
            };

            match px_msg {
                RenderMsg::Pixel { x, y, color } => {
                    fn convert(c: f32) -> u8 {
                        (c.clamp(0.0, 1.0) * u8::MAX as f32).round() as u8
                    }
                    self.frame_buffer[(x as usize, y as usize)] =
                        Color32::from_rgb(convert(color[0]), convert(color[1]), convert(color[2]))
                }
                RenderMsg::Abort => unreachable!(),
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let native_options = NativeOptions::default();
    run_native(
        "Culet Viewer",
        native_options,
        Box::new(|cc| Box::new(CuletViewer::new(&cc))),
    )
}
