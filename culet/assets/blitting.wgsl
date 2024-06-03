@group(0) @binding(0) var ray_trace_output: texture_2d<f32>;
@group(0) @binding(1) var<uniform> viewport_dimensions: vec2u;

struct FullscreenVertexOutput {
    @builtin(position)
    position: vec4f,
    @location(0)
    uv: vec2f,
}
@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> FullscreenVertexOutput {
    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
    let clip_position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return FullscreenVertexOutput(clip_position, uv);
}

@fragment
fn fragment(@builtin(position) in: vec4f) -> @location(0) vec4<f32> {
    let texture_dimensions = textureDimensions(ray_trace_output) ;
    let x_start = (i32(viewport_dimensions.x) - i32(texture_dimensions.x)) / 2;
    let y_start = (i32(viewport_dimensions.y) - i32(texture_dimensions.y)) / 2;
    return textureLoad(ray_trace_output, vec2i(i32(in.x) - x_start, i32(in.y) - y_start), 0);
}