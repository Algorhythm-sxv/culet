const MAX_ITERATIONS: u32 = 50u;

struct Triangle {
    p0: vec3<f32>,
    p1: vec3<f32>,
    p2: vec3<f32>,
    normal: vec3<f32>,
};

struct Camera {
    look_dir: vec3<f32>,
    up: vec3<f32>,
    position: vec3<f32>,
    fov_h: f32,
    aspect_ratio: f32,
    focal_length: f32,
};

@group(2)
@binding(0)
var<uniform> camera: Camera;

@group(1)
@binding(0)
var<storage, read> triangles: array<Triangle>;

@group(0)
@binding(0)
var texture: texture_storage_2d<rgba8unorm, write>;

// @compute
// @workgroup_size(1)
// fn main(@builtin(global_invocation_id) id: vec3<u32>) {
//     var final_iteration = MAX_ITERATIONS;
//     var c = vec2(
//         // Translated to put everything nicely in frame.
//         (f32(id.x) / f32(textureDimensions(texture).x)) * 3.0 - 2.25,
//         (f32(id.y) / f32(textureDimensions(texture).y)) * 3.0 - 1.5
//     );
//     var current_z = c;
//     var next_z: vec2<f32>;
//     for (var i = 0u; i < MAX_ITERATIONS; i++) {
//         next_z.x = (current_z.x * current_z.x - current_z.y * current_z.y) + c.x;
//         next_z.y = (2.0 * current_z.x * current_z.y) + c.y;
//         current_z = next_z;
//         if length(current_z) > 4.0 {
//             final_iteration = i;
//             break;
//         }
//     }
//     let value = f32(final_iteration) / f32(MAX_ITERATIONS);
//     textureStore(texture, vec2(i32(id.x), i32(id.y)), vec4(value, value, value, 1.0));
// }

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    var horizontal_distance = camera.focal_length * tan(radians(camera.fov_h / 2.0));
    var vertical_distance = horizontal_distance / camera.aspect_ratio;
    var up = normalize(cross(cross(camera.up, camera.look_dir), camera.look_dir));
    if (dot(up, camera.up) < 1e-7) {
        up = -up;
    }
    var left = normalize(cross(camera.up, camera.look_dir));
    var top_left = camera.position 
                    + camera.look_dir * camera.focal_length 
                    + left * horizontal_distance 
                    + up * vertical_distance;
    
    var pixel_x_delta = left * -2.0 * horizontal_distance / 1024.0;
    var pixel_y_delta = up * -2.0 * vertical_distance / 1024.0;

    var pixel_position = top_left + f32(id.x) * pixel_x_delta + f32(id.y) * pixel_y_delta;

    textureStore(texture, vec2(i32(id.x), i32(id.y)), vec4(pixel_position.x, pixel_position.y, -pixel_position.z, 1.0));
}
