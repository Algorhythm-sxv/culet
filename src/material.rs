use glam::Vec3;

#[derive(Copy, Clone, Debug)]
pub enum Material {
    Refractive { color: Vec3, refractive_index: f32 },
    Diffuse { color: Vec3 },
    Light { color: Vec3 },
}

impl Default for Material {
    fn default() -> Self {
        Self::Light {
            color: Vec3::default(),
        }
    }
}
impl Material {
    pub fn light() -> Self {
        Self::Light {
            color: Vec3::new(1.0, 1.0, 1.0),
        }
    }
    pub fn gem() -> Self {
        Self::Refractive {
            color: Vec3::new(1.0, 0.0, 1.0),
            refractive_index: 1.5,
        }
    }
    pub fn color(&self) -> Vec3 {
        match *self {
            Self::Refractive {
                color,
                refractive_index: _,
            } => color,
            Self::Diffuse { color } => color,
            Self::Light { color } => color,
        }
    }
}
