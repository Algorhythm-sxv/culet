@group(0) @binding(0) var<storage> vertices: array<vec3<f32>>;
@group(0) @binding(1) var<storage> indices: array<u32>;
@group(0) @binding(2) var<storage> triangle_indices: array<u32>;
@group(0) @binding(3) var<storage> bvh_nodes: array<BvhNode>;
@group(0) @binding(4) var<uniform> camera: Camera;
@group(0) @binding(5) var output: texture_storage_2d<rgba32float, read_write>;

struct BvhNode {
    aabb_min: vec3f,
    left_or_first: u32,
    aabb_max: vec3f,
    triangle_count: u32,
}

struct Camera {
    origin: vec3f,
    look_dir: vec3f,
    up: vec3f,
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


fn intersect_triangle(ray: Ray, tri_index: u32, min_distance: f32) -> HitInfo {
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

fn intersect_triangles(ray: Ray) -> HitInfo {
    var closest_hit = HitInfo(vec3f(0.0), vec3f(0.0), 1e20, false);

    for (var i = 0u; i < arrayLength(&indices) / 3u; i++) {
        let hit = intersect_triangle(ray, i, 1e-5);
        if hit.ray_distance > 1e-5 && hit.ray_distance < closest_hit.ray_distance {
            closest_hit = hit;
        }
    }

    return closest_hit;
}

fn intersect_aabb(ray: Ray, min: vec3f, max: vec3f, far_limit: f32) -> f32 {
    let tx1 = (min.x - ray.origin.x) / ray.direction.x;
    let tx2 = (max.x - ray.origin.x) / ray.direction.x;
    var tmin = min(tx1, tx2);
    var tmax = max(tx1, tx2);
    let ty1 = (min.y - ray.origin.y) / ray.direction.y;
    let ty2 = (max.y - ray.origin.y) / ray.direction.y;
    tmin = max(tmin, min(ty1, ty2));
    tmax = min(tmax, max(ty1, ty2));
    let tz1 = (min.z - ray.origin.z) / ray.direction.z;
    let tz2 = (max.z - ray.origin.z) / ray.direction.z;
    tmin = max(tmin, min(tz1, tz2));
    tmax = min(tmax, max(tz1, tz2));

    if tmax >= tmin && tmin < far_limit && tmax > 0.0 {
        // we don't mind if tmin is negative, still a ray intersection
        return tmin;
    } else {
        return 1e20;
    }
}

fn intersect_bvh(ray: Ray) -> HitInfo {
    var node_stack: array<u32, 32>;
    var node = bvh_nodes[0];
    var stack_idx = 0u;

    var closest_hit = HitInfo(vec3f(0.0), vec3f(0.0), 1e20, false);

    loop {
        // leaf nodes
        if node.triangle_count != 0 {
            // find the closest triangle intersection
            for (var i = 0u; i < node.triangle_count; i++) {
                let hit = intersect_triangle(ray, triangle_indices[node.left_or_first + i], 1e-5);
                if hit.ray_distance > 1e-5 && hit.ray_distance < closest_hit.ray_distance {
                    closest_hit = hit;
                }
            }

            // root node is a leaf, no more nodes to test
            if stack_idx == 0 {
                break;
            }

            // the ray may hit other leaf nodes, so we keep going
            stack_idx--;
            node = bvh_nodes[node_stack[stack_idx] ];
            continue;
        }
        
        // branch nodes
        let left_child = node.left_or_first;
        let right_child = node.left_or_first + 1;
        var closest_child: u32;
        var furthest_child: u32;

        let left_distance = intersect_aabb(ray, bvh_nodes[left_child].aabb_min, bvh_nodes[left_child].aabb_max, closest_hit.ray_distance);
        let right_distance = intersect_aabb(ray, bvh_nodes[right_child].aabb_min, bvh_nodes[right_child].aabb_max, closest_hit.ray_distance);
        var closest_distance: f32;
        var furthest_distance: f32;

        if left_distance > right_distance {
            closest_child = right_child;
            furthest_child = left_child;
            closest_distance = right_distance;
            furthest_distance = left_distance;
        } else {
            closest_child = left_child;
            furthest_child = right_child;
            closest_distance = left_distance;
            furthest_distance = right_distance;
        }

        if closest_distance == 1e20 {
            // ray missed both children or closest AABB intersection has been found
            if stack_idx == 0 {
                // root node, no other intersections to check
                break;
            } else {
                // we are done testing this node and its children, move up the stack
                stack_idx--;
                node = bvh_nodes[node_stack[stack_idx] ];
            }
        } else {
            // ray hit a child bounding box, test the closest child first and put the furthest on the stack for later
            node = bvh_nodes[closest_child];
            if furthest_distance != 1e20 {
                node_stack[stack_idx] = furthest_child;
                stack_idx++;
            }
        }
    }

    return closest_hit;
}

fn intersect_scene(ray: Ray) -> HitInfo {
    return intersect_bvh(ray);
}

fn lighting_model(direction: vec3f) -> vec3f {
    var cos = max(-dot(direction, camera.look_dir), 0.0);
    if degrees(acos(cos)) < 10.0 {
        cos = 0.0;
    }
    return vec3f(1.0 * cos); // TODO: configurable light intensity
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

fn trace(pixel_ray: Ray, max_depth: u32) -> vec3f {
    var refraction_colors = array<vec3f, 16>();
    var reflection_info = array<ColorListEntry, 16>();
    var reflection_color = vec3f();

    var ri = 2.16; // TODO: configurable & dispersion
    var light_color = vec3(1.0);

    let first_surface_hit = intersect_scene(pixel_ray);

    if first_surface_hit.ray_distance != 1e20 {
        reflection_info[0] = ColorListEntry(first_surface_hit.ray_distance, fresnel(pixel_ray.direction, first_surface_hit.normal, 1.0, ri));

        let first_surface_reflection = normalize(reflect(pixel_ray.direction, first_surface_hit.normal));
        reflection_color = lighting_model(first_surface_reflection) * light_color;

        var ray = Ray(first_surface_hit.position, normalize(refract(pixel_ray.direction, first_surface_hit.normal, 1.0 / ri)));
        for (var i = 1u; i < max_depth; i++) {
            let hit = intersect_scene(ray);

            if hit.ray_distance == 1e20 {
                break;
            }

            let reflection_direction = normalize(reflect(ray.direction, -hit.normal));
            let reflection_ratio = fresnel(ray.direction, -hit.normal, ri, 1.0);
            reflection_info[i] = ColorListEntry(hit.ray_distance, reflection_ratio);

            if reflection_ratio != 1.0 {
                let refraction_direction = normalize(refract(ray.direction, -hit.normal, ri));
                refraction_colors[i] = lighting_model(refraction_direction) * light_color;
            }

            ray = Ray(hit.position, reflection_direction);
        }
    } else {
        return vec3f();
    }

    var color = vec3f();

    let start = max_depth - 1;
    for (var i = start; i > 0u; i--) {
        let refraction_color = refraction_colors[i] * (1.0 - reflection_info[i].reflection_ratio);
            // TODO: configurable attenuation
        color = refraction_color + color * reflection_info[i].reflection_ratio * exp(-vec3f(0.0, 2.0, 5.0) * reflection_info[i].ray_distance);
    }

    color = reflection_color * reflection_info[0].reflection_ratio + color * (1.0 - reflection_info[0].reflection_ratio);
    return color;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let focal_length = 0.1;
    let edge_distance = focal_length * tan(camera.fov / 2.0);
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

    let color = trace(ray, 10u); // TODO: configurable max bounces
    textureStore(output, vec2(i32(id.x), i32(id.y)), vec4(color, 1.0));
}
