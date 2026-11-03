@group(0) @binding(0) var<storage, read> boxes: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> draws: array<u32>;
@group(0) @binding(2) var<uniform> planes: array<vec4<f32>, 6>;
@group(0) @binding(3) var<uniform> range: vec4<u32>;

@compute @workgroup_size(64)
fn cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= range.y {
        return;
    }
    let lo = boxes[2u * i].xyz;
    let hi = boxes[2u * i + 1u].xyz;
    for (var k = 0u; k < 6u; k++) {
        let p = planes[k];
        let v = select(lo, hi, p.xyz >= vec3(0.0));
        if dot(p.xyz, v) + p.w < 0.0 {
            draws[(range.x + i) * 5u + 1u] = 0u;
            return;
        }
    }
}
