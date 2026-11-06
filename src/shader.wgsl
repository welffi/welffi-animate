struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    tint: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var skin: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn mesh_vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>) -> MeshOut {
    var out: MeshOut;
    out.clip = u.mvp * vec4<f32>(pos, 1.0);
    out.normal = (u.model * vec4<f32>(normal, 0.0)).xyz;
    out.uv = uv;
    return out;
}

@fragment
fn mesh_fs(in: MeshOut) -> @location(0) vec4<f32> {
    let base = textureSample(skin, skin_sampler, in.uv).rgb;
    let n = normalize(in.normal);
    let key = max(dot(n, normalize(vec3<f32>(0.4, 0.8, -0.6))), 0.0);
    let fill = max(dot(n, normalize(vec3<f32>(-0.5, 0.2, 0.7))), 0.0);
    let light = 0.35 + 0.65 * key + 0.15 * fill;
    let lit = base * light;
    let shown = pow(lit, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(mix(shown, u.tint.rgb, u.tint.a), 1.0);
}

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn line_vs(@location(0) pos: vec3<f32>, @location(1) color: vec4<f32>) -> LineOut {
    var out: LineOut;
    out.clip = u.mvp * vec4<f32>(pos, 1.0);
    out.color = color;
    return out;
}

@fragment
fn line_fs(in: LineOut) -> @location(0) vec4<f32> {
    return in.color;
}
