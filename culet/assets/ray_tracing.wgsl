@group(0) @binding(0) var<storage> vertices: array<vec3<f32>>;
@group(0) @binding(1) var<storage> indices: array<u32>;

@compute(8, 8, 1)
fn main() {
}