@group(0) @binding(0) var<storage> vertices: array<vec3<f32>>;
@group(0) @binding(1) var<storage> indices: array<u32>;
@group(0) @binding(2) var<uniform> camera: Camera;
@group(0) @binding(3) var output: texture_storage_2d<rgba32float, read_write>;


struct Camera {
    origin: vec3<f32>,
    look_dir: vec3<f32>,
    up: vec3<f32>,
    fov: f32,
}

struct HitInfo {
    position: vec3f,
    normal: vec3f,
    ray_distance: f32,
    front_face: bool,
}

struct Ray {
    origin: vec3f,
    direction: vec3f,
}

fn intersect(ray: Ray, tri_index: u32, min_distance: f32) -> HitInfo {
    let i0 = indices[3u * tri_index];
    let p0 = vertices[i0];
    let i1 = indices[3u * tri_index + 1u];
    let p1 = vertices[i1];
    let i2 = indices[3u * tri_index + 2u];
    let p2 = vertices[i2];
    let edge1 = p1 - p0;
    let edge2 = p2 - p0;
    let normal = normalize(cross(edge1, edge2));

    let pvec = cross(ray.direction, edge2);
    let det = dot(edge1, pvec);

    // ray is parallel to the triangle, zero normal indicates no intersection
    if abs(det) <= 1e-7 {
        return HitInfo(vec3f(0.0), vec3f(0.0), 0.0, false);
    }

    let inv_det = 1.0 / det;
    let tvec = ray.origin - p0;
    let u = dot(tvec, pvec) * inv_det;

    // u parameter in barycentric coordinates is outside the triangle
    if u < 0.0 || u > 1.0 {
        return HitInfo(vec3f(0.0), vec3f(0.0), 0.0, false);
    }

    let qvec = cross(tvec, edge1);
    let v = dot(ray.direction, qvec) * inv_det;

    // v parameter in barycentric coordinates is outside the triangle
    if v < 0.0 || u + v > 1.0 {
        return HitInfo(vec3f(0.0), vec3f(0.0), 0.0, false);
    }

    let t = dot(edge2, qvec) * inv_det;

    if t > min_distance {
        let front_face = dot(ray.direction, normal) < 0.0;
        return HitInfo(
            ray.origin + t * ray.direction,
            normal,
            t,
            front_face,
        );
    } else {
        return HitInfo(vec3f(0.0), vec3f(0.0), 0.0, false);
    }
}

fn intersect_scene(ray: Ray) -> HitInfo {
    var closest_hit = HitInfo(vec3f(0.0), vec3f(0.0), 1e20, false);

    for (var i = 0u; i < arrayLength(&indices) / 3u; i++) {
        let hit = intersect(ray, i, 1e-5);
        if hit.ray_distance > 1e-5 && hit.ray_distance < closest_hit.ray_distance {
            closest_hit = hit;
        }
    }
    return closest_hit;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let focal_length = 0.1;
    let edge_distance = focal_length * tan(radians(camera.fov / 2.0));
    var up = normalize(cross(cross(camera.up, camera.look_dir), camera.look_dir));
    if dot(up, camera.up) < 1e-7 {
        up = -up;
    }
    let left = normalize(cross(camera.up, camera.look_dir));
    let top_left = camera.origin + camera.look_dir * focal_length + (left + up) * edge_distance;
    let pixel_x_delta = left * -2.0 * edge_distance / f32(textureDimensions(output).x);
    let pixel_y_delta = up * -2.0 * edge_distance / f32(textureDimensions(output).y);
    let pixel_position = top_left + f32(id.x) * pixel_x_delta + f32(id.y) * pixel_y_delta;

    let ray = Ray(camera.origin, normalize(pixel_position - camera.origin));

    let hit = intersect_scene(ray);
    var color = vec3(0.0);
    if hit.ray_distance != 1e20 {
        color.r = 1.0;
    }
    textureStore(output, vec2(i32(id.x), i32(id.y)), vec4(color, 1.0));
}
