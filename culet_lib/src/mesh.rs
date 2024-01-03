use std::{
    fs::OpenOptions,
    ops::{Index, Range},
    path::Path,
};

use glam::*;
use stl_io::create_stl_reader;

use crate::{
    hittable::{HitInfo, Hittable},
    material::Material,
};

#[derive(Copy, Clone, Debug)]
pub struct Triangle {
    points: [Vec3; 3],
    normal: Vec3,
    material: Material,
}

impl Index<usize> for Triangle {
    type Output = Vec3;

    fn index(&self, index: usize) -> &Self::Output {
        &self.points[index]
    }
}

impl Triangle {
    pub fn new(p1: Vec3, p2: Vec3, p3: Vec3) -> Self {
        let out = Self {
            points: [p1, p2, p3],
            normal: (p2 - p1).cross(p3 - p1).normalize(),
            material: Material::default(),
        };
        dbg!(out.normal);
        out
    }
    pub fn translate(&mut self, vector: Vec3) {
        self.points.iter_mut().for_each(|p| *p += vector)
    }
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }
}

impl From<stl_io::Triangle> for Triangle {
    fn from(value: stl_io::Triangle) -> Self {
        let p1 = Vec3::new(
            value.vertices[0][0],
            value.vertices[0][1],
            value.vertices[0][2],
        );
        let p2 = Vec3::new(
            value.vertices[1][0],
            value.vertices[1][1],
            value.vertices[1][2],
        );
        let p3 = Vec3::new(
            value.vertices[2][0],
            value.vertices[2][1],
            value.vertices[2][2],
        );
        // let normal = Vec3::new(value.normal[0], value.normal[1], value.normal[2]);
        let normal = (p2 - p1).cross(p3 - p1).normalize();
        Self {
            points: [p1, p2, p3],
            normal,
            material: Material::default(),
        }
    }
}

impl From<&stl_io::Triangle> for Triangle {
    fn from(value: &stl_io::Triangle) -> Self {
        Self::from(value.clone())
    }
}

impl Hittable for Triangle {
    fn hit_point(&self, ray: &crate::ray::Ray, min_distance: f32) -> Option<HitInfo> {
        // Möller-Trumbore intersection algorithm
        let edge01 = self[1] - self[0];
        let edge02 = self[2] - self[0];
        let pvec = ray.direction().cross(edge02);
        let determinant = edge01.dot(pvec);

        // determinant is ~= 0, triangle is parallel to the ray
        if determinant.abs() < f32::EPSILON {
            return None;
        }

        let inv_det = 1.0 / determinant;
        let tvec = ray.origin() - self[0];
        let u = tvec.dot(pvec) * inv_det;

        // u parameter in barycentric coordinates is outside of the triangle
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let qvec = tvec.cross(edge01);
        let v = ray.direction().dot(qvec) * inv_det;

        // v parameter in barycentric coordinates is outside of the triangle
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = edge02.dot(qvec) * inv_det;

        if t > min_distance {
            let front_face = ray.direction().dot(self.normal) < 0.0;
            if !front_face {
                // dbg!(ray.origin() + t * ray.direction());
            }
            Some(HitInfo {
                position: ray.origin() + t * ray.direction(),
                normal: self.normal,
                ray_distance: t,
                front_face,
                material: self.material,
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct BoundingBox {
    range_x: Range<f32>,
    range_y: Range<f32>,
    range_z: Range<f32>,
}

impl BoundingBox {
    pub fn axis(&self, n: usize) -> Range<f32> {
        match n {
            1 => self.range_y.clone(),
            2 => self.range_z.clone(),
            _ => self.range_x.clone(),
        }
    }
}

impl Hittable for BoundingBox {
    fn hit_point(&self, ray: &crate::ray::Ray, _min_distance: f32) -> Option<HitInfo> {
        let mut min_t = f32::NEG_INFINITY;
        let mut max_t = f32::INFINITY;

        for i in 0..3 {
            let inv_dir = 1.0 / ray.direction()[i];
            let origin = ray.origin()[i];

            let mut t0 = (self.axis(i).start - origin) * inv_dir;
            let mut t1 = (self.axis(i).end - origin) * inv_dir;

            if inv_dir < 0.0 {
                std::mem::swap(&mut t0, &mut t1)
            }

            // ignore minimum distances for bounding box intersections
            min_t = min_t.max(t0);
            max_t = max_t.min(t1);

            if max_t <= min_t {
                return None;
            }
        }
        Some(HitInfo {
            position: ray.origin() + min_t * ray.direction(),
            normal: Vec3::splat(0.0),
            ray_distance: min_t,
            front_face: true,
            material: Material::default(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Mesh {
    origin: Vec3,
    triangles: Vec<Triangle>,
    bounding_box: BoundingBox,
}

impl Hittable for Mesh {
    fn hit_point(
        &self,
        ray: &crate::ray::Ray,
        min_distance: f32,
    ) -> Option<crate::hittable::HitInfo> {
        if self.bounding_box.hit_by(ray, min_distance) {
            self.triangles
                .iter()
                .filter_map(|t| t.hit_point(ray, min_distance))
                .filter(|i| i.ray_distance >= min_distance)
                .min_by(|h1, h2| h1.ray_distance.partial_cmp(&h2.ray_distance).unwrap())
        } else {
            None
        }
    }
}

impl Mesh {
    pub fn load_from_stl<P: AsRef<Path>>(origin: Vec3, path: P) -> Self {
        let mut stl_file = OpenOptions::new()
            .read(true)
            .open(path.as_ref())
            .expect(&format!("File not found: {}", path.as_ref().display()));
        let stl = create_stl_reader(&mut stl_file)
            .expect(&format!("Invalid STL in file: {}", path.as_ref().display()));
        let tris: Vec<Triangle> = stl
            .map(|t| {
                t.expect(&format!(
                    "Invalid triangle in : {}",
                    path.as_ref().display()
                ))
                .into()
            })
            .collect();

        Self::from_tris_with_material(origin, tris, Material::gem())
    }
    pub fn from_tris_with_material<I, T>(origin: Vec3, tris: I, material: Material) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Triangle>,
    {
        let mut mesh = Self::from_tris(origin, tris);
        mesh.triangles
            .iter_mut()
            .for_each(|t| t.material = material);
        mesh
    }
    pub fn from_tris<I, T>(origin: Vec3, tris: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Triangle>,
    {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut max_z = f32::NEG_INFINITY;

        let tris: Vec<_> = tris
            .into_iter()
            .map(|t| {
                let mut tri = <T as Into<Triangle>>::into(t);
                tri.translate(origin);
                tri
            })
            .collect();

        for t in tris.iter() {
            for v in 0..3 {
                min_x = min_x.min(t[v][0]);
                min_y = min_y.min(t[v][1]);
                min_z = min_z.min(t[v][2]);
                max_x = max_x.max(t[v][0]);
                max_y = max_y.max(t[v][1]);
                max_z = max_z.max(t[v][2]);
            }
        }

        // don't allow BBs with zero dimensions
        Self {
            origin,
            triangles: tris,
            bounding_box: BoundingBox {
                range_x: min_x..max_x.max(min_x + 0.1),
                range_y: min_y..max_y.max(min_y + 0.1),
                range_z: min_z..max_z.max(min_z + 0.1),
            },
        }
    }
}
