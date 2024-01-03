use std::fs::OpenOptions;

use anyhow::Result;
use glam::vec3;
use image::*;
use stl_io::create_stl_reader;

mod camera;
mod hittable;
mod material;
mod mesh;
mod ray;
mod render;
mod scene;

use crate::{
    camera::Camera,
    material::Material,
    mesh::{Mesh, Triangle},
    render::{gamma_correct, RenderInfo},
    scene::Scene,
};

fn main() -> Result<()> {
    let mut stl_file = OpenOptions::new().read(true).open("lowboy.stl")?;
    let stl = create_stl_reader(&mut stl_file)?;

    let tris: Vec<_> = stl.map(|r| r.expect("Triangle read error!")).collect();
    println!("Tris: {}", tris.len());

    let camera = Camera::default()
        .fov(12.0)
        .position(vec3(0.2, 0.0, 10.0))
        .look_at(vec3(0.0, 0.0, -1.5))
        .aspect_ratio(1.0);
    let scene = Scene::new(vec![Mesh::from_tris_with_material(
        vec3(0.0, 0.0, -1.5),
        tris,
        Material::gem(),
    )]);
    // let test_scene = Scene::new(vec![
    //     Mesh::from_tris_with_material(
    //         vec3(-1.0, -1.0, -1.0),
    //         vec![
    //             Triangle::new(
    //                 vec3(0.0, 0.0, 0.0),
    //                 vec3(1.0, 0.0, 0.0),
    //                 vec3(0.0, 0.0, -10.0),
    //             ),
    //             Triangle::new(
    //                 vec3(0.0, 0.0, -10.0),
    //                 vec3(1.0, 0.0, 0.0),
    //                 vec3(1.0, 0.0, -10.0),
    //             ),
    //         ],
    //         Material::light(),
    //     ),
    //     Mesh::from_tris_with_material(
    //         vec3(-0.1, -1.0, -1.0),
    //         vec![
    //             Triangle::new(
    //                 vec3(0.0, 0.0, 0.0),
    //                 vec3(1.0, 0.0, 0.0),
    //                 vec3(0.0, 1.0, -0.5),
    //             ),
    //             Triangle::new(
    //                 vec3(0.0, 1.0, -0.5),
    //                 vec3(1.0, 0.0, 0.0),
    //                 vec3(1.0, 1.0, -0.5),
    //             ),
    //         ],
    //         Material::gem(),
    //     ),
    // ]);

    let render_config = RenderInfo::new()
        .camera(camera)
        .scene(scene)
        .samples_per_pixel(1)
        .max_bounces(8)
        .image_width(720)
        .image_height(720);
    let pixels = render_config.render();

    let output_image = RgbImage::from_vec(
        720,
        720,
        pixels
            .into_iter()
            .flat_map(gamma_correct)
            .map(|f| (f.clamp(0.0, 1.0) * (u8::MAX as f32)).round() as u8)
            .collect::<Vec<u8>>(),
    )
    .unwrap();

    output_image.save("output.png")?;

    Ok(())
}
