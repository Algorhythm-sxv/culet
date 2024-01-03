use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::*,
    Arc,
};

use glam::{vec3, Vec3};
use rand::{rngs::SmallRng, seq::SliceRandom, Rng, SeedableRng};
use rayon::ThreadPoolBuilder;

use crate::{camera::Camera, hittable::Hittable, material::Material, ray::Ray, scene::Scene};

pub enum RenderMsg {
    Pixel { x: u32, y: u32, color: Vec3 },
    Abort,
}

#[derive(Clone, Debug)]
pub struct AbortSignal(Arc<AtomicBool>);

impl AbortSignal {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    pub fn abort(&self) {
        self.0.store(true, Ordering::Relaxed)
    }
    pub fn is_aborted(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum LightingModel {
    Isometric,
    Cosine,
}

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub camera: Camera,
    pub scene: Arc<Scene>,
    pub image_width: usize,
    pub image_height: usize,
    pub samples_per_pixel: usize,
    pub max_bounces: usize,
    pub lighting_model: LightingModel,
    pub background_color: Vec3,
    pub threads: usize,
}

impl RenderOptions {
    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
            scene: Arc::new(Scene::empty()),
            image_width: 1280,
            image_height: 720,
            samples_per_pixel: 1,
            max_bounces: 1,
            lighting_model: LightingModel::Cosine,
            background_color: Vec3::splat(0.1),
            threads: 1,
        }
    }
    pub fn camera(mut self, camera: Camera) -> Self {
        self.camera = camera;
        self
    }
    pub fn scene(mut self, scene: Arc<Scene>) -> Self {
        self.scene = scene;
        self
    }
    pub fn image_width(mut self, image_width: usize) -> Self {
        self.image_width = image_width;
        self
    }
    pub fn image_height(mut self, image_height: usize) -> Self {
        self.image_height = image_height;
        self
    }

    pub fn samples_per_pixel(mut self, samples: usize) -> Self {
        self.samples_per_pixel = samples;
        self
    }

    pub fn max_bounces(mut self, bounces: usize) -> Self {
        self.max_bounces = bounces;
        self
    }

    pub fn background_color(mut self, background_color: Vec3) -> Self {
        self.background_color = background_color;
        self
    }

    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    pub fn render_streaming(&self) -> (Receiver<RenderMsg>, AbortSignal) {
        let mut pixels: Vec<usize> = (0..self.image_width * self.image_height).collect();

        let (top_left, viewport_width, viewport_height) = self.camera.viewport();
        let pixel_x_delta = viewport_width / self.image_width as f32;
        let pixel_y_delta = viewport_height / self.image_height as f32;

        let mut rng = SmallRng::seed_from_u64(0x123456789ABCDEF);
        pixels.shuffle(&mut rng);

        let (tx, rx) = channel();

        let thread_pool = ThreadPoolBuilder::new()
            .num_threads(self.threads)
            .build()
            .unwrap();

        let abort_signal = AbortSignal::new();

        pixels.chunks(self.threads).for_each(|chunk| {
            let mut rng = rng.clone();
            let tx = tx.clone();
            let options = self.clone();
            let chunk = chunk.to_vec();
            let abort_signal = abort_signal.clone();

            thread_pool.spawn(move || {
                for i in chunk {
                    if abort_signal.is_aborted() {
                        break;
                    }
                    let x = i % options.image_width;
                    let y = i / options.image_width;
                    let mut pixel = Vec3::default();
                    for i in 0..options.samples_per_pixel {
                        let mut pixel_position = top_left
                            + (x as f32 + 0.5) * pixel_x_delta
                            + (y as f32 + 0.5) * pixel_y_delta;
                        if i != 0 {
                            let x_jitter = rng.gen_range(-0.5..0.5);
                            let y_jitter = rng.gen_range(-0.5..0.5);
                            pixel_position += x_jitter * pixel_x_delta + y_jitter * pixel_y_delta;
                        }
                        let ray = Ray::new(
                            options.camera.position,
                            pixel_position - options.camera.position,
                        );
                        pixel += options.trace(&ray, options.max_bounces);
                    }
                    let _ = tx.send(RenderMsg::Pixel {
                        x: x as u32,
                        y: y as u32,
                        color: pixel / options.samples_per_pixel as f32,
                    });
                }
            });
        });

        (rx, abort_signal)
    }

    pub fn trace(&self, ray: &Ray, max_bounces: usize) -> Vec3 {
        match self.scene.hit_point(ray, 1e-5) {
            Some(info) => {
                if max_bounces == 0 {
                    return info.material.color() * 0.0;
                }
                match info.material {
                    Material::Refractive {
                        color,
                        refractive_index,
                    } => {
                        let normal = if info.front_face {
                            info.normal
                        } else {
                            -info.normal
                        };
                        let reflection_ratio = fresnel(ray.direction(), normal, refractive_index);

                        let exiting_pavilion =
                            !info.front_face && normal.dot(vec3(0.0, 0.0, 1.0)) > 0.0;
                        // color from refraction ray
                        let refraction_color = if reflection_ratio < 1.0 && !exiting_pavilion {
                            let ri_ratio = if info.front_face {
                                1.0 / refractive_index
                            } else {
                                refractive_index
                            };
                            let cos_1 = (-ray.direction()).dot(normal).min(1.0);
                            // refraction term
                            let out_perp = ri_ratio * (ray.direction() + cos_1 * normal);
                            let out_parallel =
                                normal * -(1.0 - out_perp.length_squared().min(1.0)).sqrt();

                            let out_direction = (out_perp + out_parallel).normalize();
                            let out_origin = info.position;

                            self.trace(&Ray::new(out_origin, out_direction), max_bounces - 1)
                        } else {
                            Vec3::splat(0.0)
                        };

                        // color from reflection ray
                        let reflection_color = {
                            let out_direction = (ray.direction()
                                - 2.0 * ray.direction().dot(normal) * normal)
                                .normalize();
                            let out_origin = info.position;

                            self.trace(&Ray::new(out_origin, out_direction), max_bounces - 1)
                        };

                        let material_reflectance = if !info.front_face { 1.0 } else { 3e-6 };
                        let subcolor = reflection_ratio * reflection_color * material_reflectance
                            + (1.0 - reflection_ratio) * refraction_color;

                        if !info.front_face {
                            subcolor * ((Vec3::splat(1.0) - color) * -0.5 * info.ray_distance).exp()
                        } else {
                            subcolor
                        }
                    }
                    Material::Diffuse { color: _ } => todo!(),
                    Material::Light { color } => color,
                }
            }
            None => {
                if max_bounces == self.max_bounces {
                    self.background_color
                } else {
                    match self.lighting_model {
                        LightingModel::Cosine => {
                            let mut cos = -ray.direction().dot(self.camera.look_dir()).min(0.0);
                            // add a head shadow directly above
                            if cos.acos().to_degrees() < 24.0 {
                                cos = 0.0;
                            }
                            Vec3::splat(100000.0) * cos
                        }
                        LightingModel::Isometric => {
                            if ray.direction().dot(-self.camera.look_dir()) >= 0.0 {
                                Vec3::splat(1.0)
                            } else {
                                Vec3::splat(0.0)
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn gamma_correct(color: Vec3) -> [f32; 3] {
    [
        color[0].powf(2.2f32.recip()),
        color[1].powf(2.2f32.recip()),
        color[2].powf(2.2f32.recip()),
    ]
}

// calculate the proportion of color that should come from reflection vs refraction
fn fresnel(incoming: Vec3, normal: Vec3, refractive_index: f32) -> f32 {
    let cos_i = incoming.dot(normal);
    let (eta_i, eta_t) = if cos_i > 0.0 {
        (refractive_index, 1.0)
    } else {
        (1.0, refractive_index)
    };

    let sin_t = (eta_i / eta_t) * (1.0 - cos_i * cos_i).max(0.0).sqrt();
    if sin_t > 1.0 {
        // total internal reflection
        1.0
    } else {
        let cos_t = (1.0 - sin_t * sin_t).max(0.0).sqrt();
        let cos_i = cos_i.abs();
        let r_s = ((eta_t * cos_i) - (eta_i * cos_t)) / ((eta_t * cos_i) + (eta_i * cos_t));
        let r_p = ((eta_i * cos_i) - (eta_t * cos_t)) / ((eta_i * cos_i) + (eta_t * cos_t));

        (r_s * r_s + r_p * r_p) / 2.0
    }
}
