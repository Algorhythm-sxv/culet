use std::sync::{mpsc::Receiver, Arc};

use culet_lib::{
    camera::Camera,
    glam::{vec3, Mat3, Vec3},
    mesh::Mesh,
    render::{AbortSignal, RenderMsg, RenderOptions},
    scene::Scene,
};
use eframe::{run_native, App, CreationContext, NativeOptions, Renderer};
use egui::{
    load::SizedTexture, CentralPanel, Color32, ColorImage, DragValue, ImageSource, RichText,
    ScrollArea, Sense, SidePanel, Slider, TextureHandle, TextureOptions, Vec2, ViewportBuilder,
};

const DEFAULT_SIZE: usize = 400;

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
            .threads(
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1),
            )
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

        let mut rotation_changed = false;
        let mut resolution_changed = false;
        let mut bounces_changed = false;
        let mut lighting_changed = false;
        let mut color_changed = false;
        let mut ri_changed = false;

        // settings panel
        SidePanel::right(egui::Id::new("Settings panel")).show(ctx, |ui| {
            ui.vertical(|ui| {
                // render size
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Image Size").heading());
                    let resp = ui.add(
                        DragValue::new(&mut self.render_options.image_width)
                            .clamp_range(0..=800)
                            .speed(1.0),
                    );

                    resolution_changed = resp.changed();
                });

                ui.separator();

                // render threads
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Threads").heading());
                    ui.add(Slider::new(
                        &mut self.render_options.threads,
                        1..=std::thread::available_parallelism()
                            .map(|n| n.get())
                            .unwrap_or(1),
                    ))
                });

                ui.separator();

                // max bounces
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Max Bounces").heading());
                    let resp = ui.add(Slider::new(&mut self.render_options.max_bounces, 1..=20));

                    bounces_changed = resp.changed();
                });

                ui.separator();

                // gem color
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Gem Color").heading());
                    ui.vertical(|ui| {
                        ui.label("Red");
                        let resp_red = ui.add(
                            Slider::new(&mut self.render_options.gem_color[0], 0.0..=10.0)
                                .drag_value_speed(0.001),
                        );
                        ui.label("Green");
                        let resp_green = ui.add(
                            Slider::new(&mut self.render_options.gem_color[1], 0.0..=10.0)
                                .drag_value_speed(0.001),
                        );
                        ui.label("Blue");
                        let resp_blue = ui.add(
                            Slider::new(&mut self.render_options.gem_color[2], 0.0..=10.0)
                                .drag_value_speed(0.001),
                        );

                        color_changed =
                            resp_red.changed() || resp_blue.changed() || resp_green.changed();
                    });
                });

                ui.separator();

                // refractive index
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Refractive Index").heading());
                    let resp = ui.add(
                        Slider::new(&mut self.render_options.gem_ri, 1.0..=3.0)
                            .drag_value_speed(0.001),
                    );
                    ri_changed = resp.changed();
                });

                ui.separator();

                // light intensity
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Light Intensity").heading());
                    let resp = ui.add(Slider::new(
                        &mut self.render_options.light_intensity,
                        0.1..=5.0,
                    ));

                    lighting_changed = resp.changed();
                });
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    let image = egui::Image::new(ImageSource::Texture(SizedTexture::from_handle(
                        &self.render_buffer_handle,
                    )))
                    .fit_to_exact_size(Vec2::splat(2.0 * DEFAULT_SIZE as f32))
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
                        rotation_changed = true;
                    }
                });
            });
        });

        if resolution_changed {
            self.render_options.image_height = self.render_options.image_width;
        }

        if color_changed || ri_changed {
            let mut new_scene = (*self.render_options.scene).clone();
            new_scene.meshes_mut().for_each(|m| {
                if color_changed {
                    m.apply_color(self.render_options.gem_color);
                }
                if ri_changed {
                    m.apply_ri(self.render_options.gem_ri);
                }
            });

            self.render_options.scene = Arc::new(new_scene);
        }

        let render_dirty = rotation_changed
            || resolution_changed
            || bounces_changed
            || color_changed
            || ri_changed
            || lighting_changed;
        if render_dirty {
            // abort previous render
            self.render_abort.abort();

            // clear frame buffer
            // if resolution_changed {
            self.frame_buffer = ColorImage::new(
                [
                    self.render_options.image_width,
                    self.render_options.image_height,
                ],
                Color32::BLACK,
            );
            // }

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

            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(puffin)]
    {
        let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
        let _puffin_server = puffin_http::Server::new(&server_addr).unwrap();
        puffin::set_scopes_on(true);
    }

    let native_options = NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size((1200.0, 850.0)),
        renderer: Renderer::Wgpu,
        ..Default::default()
    };
    run_native(
        "Culet Viewer",
        native_options,
        Box::new(|cc| Box::new(CuletViewer::new(cc))),
    )
}
