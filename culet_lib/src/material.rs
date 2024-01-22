use glam::Vec3;

pub const DEFAULT_GEM_COLOR: Vec3 = Vec3::new(0.0, 0.0, 0.0);
pub const DEFAULT_GEM_RI: f32 = 1.54;
pub const DEFAULT_GEM_DISPERSION: f32 = 0.008;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Material {
    Refractive {
        color: Vec3,
        refractive_index: f32,
        dispersion: f32,
    },
    Diffuse {
        color: Vec3,
    },
    Light {
        color: Vec3,
    },
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
            color: DEFAULT_GEM_COLOR,
            refractive_index: DEFAULT_GEM_RI,
            dispersion: DEFAULT_GEM_DISPERSION,
        }
    }
    pub fn color(&self) -> Vec3 {
        match *self {
            Self::Refractive {
                color,
                refractive_index: _,
                dispersion: _,
            }
            | Self::Diffuse { color }
            | Self::Light { color } => color,
        }
    }
}
