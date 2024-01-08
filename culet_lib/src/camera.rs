use bytemuck::{Pod, Zeroable};
use glam::*;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Camera {
    // align 16
    look_dir: Vec3,
    _pad_0: f32,
    // align 16
    up: Vec3,
    _pad_1: f32,
    // align 16
    pub position: Vec3,
    // align 4
    fov_h: f32,
    aspect_ratio: f32,
    pub focal_length: f32,
    _pad_2: f32,
    _pad_3: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            look_dir: Vec3::new(0.0, 0.0, -1.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            position: Vec3::new(0.0, 0.0, 0.0),
            fov_h: 90.0,
            aspect_ratio: 16.0 / 9.0,
            focal_length: 1.0,
            _pad_0: 0.0,
            _pad_1: 0.0,
            _pad_2: 0.0,
            _pad_3: 0.0,
        }
    }
}

impl Camera {
    pub fn new(
        position: Vec3,
        look_dir: Vec3,
        up: Vec3,
        fov_h: f32,
        aspect_ratio: f32,
        focal_length: f32,
    ) -> Self {
        assert!(
            look_dir.cross(up).length() > f32::EPSILON,
            "Camera direction and up vector must not be opposite"
        );
        Self {
            look_dir: look_dir.normalize(),
            up: up.normalize(),
            position,
            fov_h,
            aspect_ratio,
            focal_length,
            _pad_0: 0.0,
            _pad_1: 0.0,
            _pad_2: 0.0,
            _pad_3: 0.0,
        }
    }
    pub fn viewport(&self) -> (Vec3, Vec3, Vec3) {
        let horizontal_distance = self.focal_length * (self.fov_h / 2.0).to_radians().tan();
        let vertical_distance = horizontal_distance / self.aspect_ratio;

        let mut up = self
            .up
            .cross(self.look_dir)
            .cross(self.look_dir)
            .normalize();
        if up.dot(self.up) <= f32::EPSILON {
            up = -up;
        }
        let left = self.up.cross(self.look_dir).normalize();

        (
            self.position
                + self.look_dir * self.focal_length
                + left * horizontal_distance
                + up * vertical_distance,
            left * -2.0 * horizontal_distance,
            up * -2.0 * vertical_distance,
        )
    }
    pub fn position(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }
    pub fn look_at(mut self, point: Vec3) -> Self {
        self.look_dir = point - self.position;
        assert!(
            self.look_dir.length() > self.focal_length,
            "Camera observation point must be further than the focal distance"
        );

        self.look_dir = self.look_dir.normalize();
        self
    }
    pub fn fov(mut self, fov: f32) -> Self {
        self.fov_h = fov;
        self
    }
    pub fn aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = aspect_ratio;
        self
    }
    pub fn look_dir(&self) -> Vec3 {
        self.look_dir
    }
}
