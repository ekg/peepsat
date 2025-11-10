@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);  // Red color
}

struct Uniforms {
    mvp: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn globe_vs(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return uniforms.mvp * vec4<f32>(position, 1.0);
}

@fragment
fn globe_fs() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 1.0, 0.0, 0.5); // semi-transparent green
}