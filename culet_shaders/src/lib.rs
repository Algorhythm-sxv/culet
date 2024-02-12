#![no_std]

use spirv_std::glam::{uvec2, vec4, UVec3};
use spirv_std::{spirv, Image};

#[spirv(compute(threads(64)))]
pub fn main_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(access_qualifier = writeonly, descriptor_set = 0, binding = 0)] image: &Image!(2D, format = rgba8, sampled = false),
) {
    unsafe {
        image.write(
            uvec2(id.x, id.y),
            vec4(id.x as f32, id.y as f32, id.z as f32, 1.0),
        );
    }
}
