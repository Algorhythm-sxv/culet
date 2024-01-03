use glam::*;

use crate::{material::Material, ray::Ray};

pub trait Hittable {
    fn hit_point(&self, ray: &Ray, min_distance: f32) -> Option<HitInfo>;
    fn hit_by(&self, ray: &Ray, min_distance: f32) -> bool {
        self.hit_point(ray, min_distance).is_some()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct HitInfo {
    pub position: Vec3,
    pub normal: Vec3,
    pub ray_distance: f32,
    pub front_face: bool,
    pub material: Material,
}
