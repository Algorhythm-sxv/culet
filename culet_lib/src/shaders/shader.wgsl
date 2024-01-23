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

struct RenderInfo {
    attenuation: vec3f,
    max_bounces: u32,
    refractive_index: f32,
    light_intensity: f32,
}

@group(3)
@binding(0)
var<uniform> render_info: RenderInfo;

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

fn intersect_scene(ray: Ray) -> HitInfo {
    var closest_hit = HitInfo(vec3f(0.0), vec3f(0.0), 1e20, false);

    for (var i = 0u; i < arrayLength(&triangles); i++) {
        let hit = intersect(ray, triangles[i], 1e-5);
        if hit.ray_distance > 1e-5 && hit.ray_distance < closest_hit.ray_distance {
            closest_hit = hit;
        }
    }
    return closest_hit;
}

fn lighting_model(direction: vec3f) -> vec3f {
    var cos = max(-dot(direction, camera.look_dir), 0.0);
    if degrees(acos(cos)) < 10.0 {
        cos = 0.0;
    }
    return vec3f(render_info.light_intensity * cos);
}

fn fresnel(incoming: vec3f, normal: vec3f, eta_i: f32, eta_t: f32) -> f32 {
    let cos_i = dot(incoming, normal);

    let sin_t = (eta_i / eta_t) * sqrt(max((1.0 - cos_i * cos_i), 0.0));
    if sin_t > 1.0 {
        return 1.0;
    } else {
        let cos_t = sqrt(max(1.0 - sin_t * sin_t, 0.0));
        let cos_i = abs(cos_i);
        let r_s = ((eta_i * cos_i) - (eta_t * cos_t)) / ((eta_i * cos_i) + (eta_t * cos_t));
        let r_p = ((eta_i * cos_t) - (eta_t * cos_i)) / ((eta_i * cos_i) + (eta_t * cos_i));

        return (r_s * r_s + r_p * r_p) / 2.0;
    }
}

struct ColorListEntry {
    ray_distance: f32,
    reflection_ratio: f32,
}

fn trace(pixel_ray: Ray, max_depth: i32) -> vec3f {
    var refraction_colors = array<vec3f, 16>();
    var reflection_info = array<ColorListEntry, 16>();
    var reflection_color = vec3f();

    let first_surface_hit = intersect_scene(pixel_ray);

    // ray hit the gem
    if first_surface_hit.ray_distance != 1e20 {
        reflection_info[0] = ColorListEntry(first_surface_hit.ray_distance,
            fresnel(pixel_ray.direction, first_surface_hit.normal, 1.0, render_info.refractive_index));

        let first_surface_reflection = normalize(reflect(pixel_ray.direction, first_surface_hit.normal));
        reflection_color = lighting_model(first_surface_reflection);

        // this is the only refraction ray that can bounce around, all others should miss
        var ray = Ray(first_surface_hit.position, normalize(refract(pixel_ray.direction, first_surface_hit.normal, 1.0 / render_info.refractive_index)));
        for (var i = 1; i < max_depth; i++) {
            let hit = intersect_scene(ray);

            if hit.ray_distance == 1e20 {
                // reflection ray clipped out of the gem and missed
                // return vec3f(1.0, 0.0, 0.0);
                break;
            }

            let reflection_direction = normalize(reflect(ray.direction, -hit.normal));
            // internal reflection ratio
            let reflection_ratio = fresnel(ray.direction, -hit.normal, render_info.refractive_index, 1.0);
            reflection_info[i] = ColorListEntry(hit.ray_distance, reflection_ratio);

            // calculate color from escaping rays
            if reflection_ratio != 1.0 {
                let refraction_direction = normalize(refract(ray.direction, -hit.normal, render_info.refractive_index));
                refraction_colors[i] = lighting_model(refraction_direction);
            }

            ray = Ray(hit.position, reflection_direction);
        }
    } else {
        return vec3f();
    }

    var color = vec3f();
    // walk back through the bounces and accumulate the color
    var start = max_depth - 1;
    for (var i = max_depth - 1; i > 0; i--) {
        let refraction_color = refraction_colors[i] * (1.0 - reflection_info[i].reflection_ratio);

        // reflection color gets attenuated wrt Beer's law and mixed with the reflection ratio
        color = refraction_color + color * reflection_info[i].reflection_ratio * exp(-render_info.attenuation * reflection_info[i].ray_distance);
    }

    // blend first reflection and refraction
    color = reflection_color * reflection_info[0].reflection_ratio + color * (1.0 - reflection_info[0].reflection_ratio);
    return color;
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

    let color = trace(ray, i32(render_info.max_bounces));

    textureStore(texture, vec2(i32(id.x), i32(id.y)), vec4(color, 1.0));
}
