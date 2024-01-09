struct Triangle {
    p0: vec3f,
    p1: vec3f,
    p2: vec3f,
    normal: vec3f,
};

@group(1)
@binding(0)
var<storage, read> triangles: array<Triangle>;

struct Camera {
    look_dir: vec3f,
    up: vec3f,
    position: vec3f,
    fov_h: f32,
    aspect_ratio: f32,
    focal_length: f32,
};

@group(2)
@binding(0)
var<uniform> camera: Camera;

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

fn intersect(ray: Ray, tri: Triangle, min_distance: f32) -> HitInfo {
    let edge1 = tri.p1 - tri.p0;
    let edge2 = tri.p2 - tri.p0;

    let pvec = cross(ray.direction, edge2);
    let det = dot(edge1, pvec);

    // ray is parallel to the triangle, zero normal indicates no intersection
    if abs(det) <= 1e-7 {
        return HitInfo(vec3f(0.0), vec3f(0.0), 0.0, false);
    }

    let inv_det = 1.0 / det;
    let tvec = ray.origin - tri.p0;
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
        let front_face = dot(ray.direction, tri.normal) < 0.0;
        return HitInfo(
            ray.origin + t * ray.direction,
            tri.normal,
            t,
            front_face,
        );
    } else {
        return HitInfo(vec3f(0.0), vec3f(0.0), 0.0, false);
    }
}

@group(0)
@binding(0)
var texture: texture_storage_2d<rgba8unorm, write>;

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let horizontal_distance = camera.focal_length * tan(radians(camera.fov_h / 2.0));
    let vertical_distance = horizontal_distance / camera.aspect_ratio;
    var up = normalize(cross(cross(camera.up, camera.look_dir), camera.look_dir));
    if dot(up, camera.up) < 1e-7 {
        up = -up;
    }
    let left = normalize(cross(camera.up, camera.look_dir));
    let top_left = camera.position + camera.look_dir * camera.focal_length + left * horizontal_distance + up * vertical_distance;

    let pixel_x_delta = left * -2.0 * horizontal_distance / f32(textureDimensions(texture).x);
    let pixel_y_delta = up * -2.0 * vertical_distance / f32(textureDimensions(texture).y);

    let pixel_position = top_left + f32(id.x) * pixel_x_delta + f32(id.y) * pixel_y_delta;

    let ray = Ray(camera.position, normalize(pixel_position - camera.position));

    var color = vec3f(0.0);
    var closest_hit = HitInfo(vec3f(0.0), vec3f(0.0), 1e20, false);

    for (var i = 0u; i < arrayLength(&triangles); i++) {
        let hit = intersect(ray, triangles[i], 1e-3);
        if hit.ray_distance > 1e-3 && hit.ray_distance < closest_hit.ray_distance {
            closest_hit = hit;
        }
    }
    color.x = -dot(closest_hit.normal, ray.direction);

    textureStore(texture, vec2(i32(id.x), i32(id.y)), vec4(color, 1.0));
}
