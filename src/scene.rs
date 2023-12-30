use crate::{hittable::Hittable, mesh::Mesh, ray::Ray};

pub struct Scene {
    meshes: Vec<Mesh>,
    shadow_bias: f32,
}

impl Hittable for Scene {
    fn hit_point(&self, ray: &Ray, min_distance: f32) -> Option<crate::hittable::HitInfo> {
        let mut closest_hit_distance = f32::INFINITY;
        let mut closest_hit_info = None;
        for mesh in self.meshes.iter() {
            if let Some(info) = mesh.hit_point(ray, min_distance) {
                if info.ray_distance < closest_hit_distance {
                    closest_hit_distance = info.ray_distance;
                    closest_hit_info = Some(info);
                }
            }
        }
        closest_hit_info
    }
}
impl Scene {
    pub fn new(meshes: Vec<Mesh>) -> Self {
        Self {
            meshes,
            shadow_bias: 1e-6,
        }
    }
    pub fn empty() -> Self {
        Self {
            meshes: vec![],
            shadow_bias: 1e-6,
        }
    }
    pub fn shadow_bias(&self) -> f32 {
        self.shadow_bias
    }
}
