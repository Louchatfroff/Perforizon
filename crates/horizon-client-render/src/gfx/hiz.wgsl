struct Frame {
    view_proj: mat4x4<f32>,
    prev_inverse: mat4x4<f32>,
    delta: vec4<f32>,
    cells: vec4<u32>,
    counts: vec4<u32>,
    levels: array<vec4<u32>, 12>,
}

const MARGIN = 0.01;
const BOX_NEAR = 0.05;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read_write> splat: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> pyramid: array<f32>;
@group(0) @binding(3) var<storage, read_write> prev: array<f32>;
@group(0) @binding(4) var<uniform> step: vec4<u32>;
@group(0) @binding(6) var depth: texture_depth_2d;
@group(0) @binding(8) var<storage, read> tests: array<vec4<u32>>;
@group(0) @binding(10) var<storage, read_write> draws: array<u32>;
@group(0) @binding(11) var<storage, read_write> rescue: array<vec4<u32>>;
@group(0) @binding(12) var<storage, read_write> args: array<atomic<u32>>;
@group(0) @binding(13) var<storage, read> origins: array<vec4<f32>>;

@compute @workgroup_size(8, 8)
fn snapshot(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = frame.cells.x;
    if id.x >= w || id.y >= frame.cells.y {
        return;
    }
    let full = frame.cells.zw;
    let x0 = id.x * 2u;
    let y0 = id.y * 2u;
    let x1 = min(x0 + 1u, full.x - 1u);
    let y1 = min(y0 + 1u, full.y - 1u);
    let a = min(textureLoad(depth, vec2(x0, y0), 0), textureLoad(depth, vec2(x1, y0), 0));
    let b = min(textureLoad(depth, vec2(x0, y1), 0), textureLoad(depth, vec2(x1, y1), 0));
    prev[id.y * w + id.x] = min(a, b);
}

@compute @workgroup_size(8, 8)
fn reproject(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = frame.cells.x;
    let h = frame.cells.y;
    if id.x >= w || id.y >= h {
        return;
    }
    let d = prev[id.y * w + id.x];
    if d <= 0.0 {
        return;
    }
    let full = vec2<f32>(frame.cells.zw);
    let uv = (vec2<f32>(id.xy) * 2.0 + 1.0) / full;
    let p = frame.prev_inverse * vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    if abs(p.w) < 1e-12 {
        return;
    }
    let c = frame.view_proj * vec4(p.xyz / p.w + frame.delta.xyz, 1.0);
    if c.w <= 1e-4 {
        return;
    }
    let n = c.xyz / c.w;
    if n.z <= 0.0 || n.z > 1.0 {
        return;
    }
    let x = i32(floor((n.x * 0.5 + 0.5) * full.x * 0.5));
    let y = i32(floor((0.5 - n.y * 0.5) * full.y * 0.5));
    if x < 0 || y < 0 || x >= i32(w) || y >= i32(h) {
        return;
    }
    let z = n.z * (1.0 - MARGIN);
    atomicMax(&splat[u32(y) * w + u32(x)], 0xffffffffu - bitcast<u32>(z));
}

fn splat_depth(i: u32) -> f32 {
    let k = atomicLoad(&splat[i]);
    return select(0.0, bitcast<f32>(0xffffffffu - k), k != 0u);
}

@compute @workgroup_size(8, 8)
fn fill(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = frame.cells.x;
    let h = frame.cells.y;
    if id.x >= w || id.y >= h {
        return;
    }
    let i = id.y * w + id.x;
    var z = splat_depth(i);
    if z == 0.0 {
        var lo = 1e30;
        var hi = 0.0;
        var n = 0u;
        for (var k = 0u; k < 4u; k++) {
            let dx = select(0, select(-1, 1, (k & 1u) != 0u), k < 2u);
            let dy = select(0, select(-1, 1, (k & 1u) != 0u), k >= 2u);
            let x = i32(id.x) + dx;
            let y = i32(id.y) + dy;
            if x < 0 || y < 0 || x >= i32(w) || y >= i32(h) {
                continue;
            }
            let v = splat_depth(u32(y) * w + u32(x));
            if v > 0.0 {
                lo = min(lo, v);
                hi = max(hi, v);
                n++;
            }
        }
        if n >= 3u && hi < lo * 1.1 {
            z = lo;
        }
    }
    pyramid[frame.levels[0].x + i] = z;
}

@compute @workgroup_size(8, 8)
fn reduce(@builtin(global_invocation_id) id: vec3<u32>) {
    let s = frame.levels[step.x];
    let d = frame.levels[step.x + 1u];
    if id.x >= d.y || id.y >= d.z {
        return;
    }
    let x0 = id.x * 2u;
    let y0 = id.y * 2u;
    let x1 = min(x0 + 1u, s.y - 1u);
    let y1 = min(y0 + 1u, s.z - 1u);
    let a = min(pyramid[s.x + y0 * s.y + x0], pyramid[s.x + y0 * s.y + x1]);
    let b = min(pyramid[s.x + y1 * s.y + x0], pyramid[s.x + y1 * s.y + x1]);
    pyramid[d.x + id.y * d.y + id.x] = min(a, b);
}

fn box_bounds(packed: u32) -> array<vec3<f32>, 2> {
    let lo = vec3<u32>(packed & 31u, (packed >> 5u) & 31u, (packed >> 10u) & 31u);
    let hi = vec3<u32>((packed >> 15u) & 31u, (packed >> 20u) & 31u, (packed >> 25u) & 31u);
    return array(vec3<f32>(lo) - 2.0, vec3<f32>(hi) - 2.0);
}

fn corner(origin: vec3<f32>, b: array<vec3<f32>, 2>, c: u32) -> vec3<f32> {
    return origin + vec3(
        select(b[0].x, b[1].x, (c & 1u) != 0u),
        select(b[0].y, b[1].y, (c & 2u) != 0u),
        select(b[0].z, b[1].z, (c & 4u) != 0u),
    );
}

fn hidden(origin: vec3<f32>, packed: u32) -> bool {
    let bounds = box_bounds(packed);
    var lo = vec2(1e30);
    var hi = vec2(-1e30);
    var nearest = -1e30;
    for (var c = 0u; c < 8u; c++) {
        let p = frame.view_proj * vec4(corner(origin, bounds, c), 1.0);
        if p.w <= BOX_NEAR {
            return false;
        }
        let n = p.xyz / p.w;
        lo = min(lo, n.xy);
        hi = max(hi, n.xy);
        nearest = max(nearest, n.z);
    }
    if hi.x < -1.0 || lo.x > 1.0 || hi.y < -1.0 || lo.y > 1.0 {
        return false;
    }
    lo = max(lo, vec2(-1.0));
    hi = min(hi, vec2(1.0));
    let cells = vec2<f32>(frame.cells.xy);
    let first = floor(vec2(lo.x * 0.5 + 0.5, 0.5 - hi.y * 0.5) * cells) - 1.0;
    let last = floor(vec2(hi.x * 0.5 + 0.5, 0.5 - lo.y * 0.5) * cells) + 1.0;
    let size = max(last.x - first.x, last.y - first.y) + 1.0;
    let level = min(u32(ceil(log2(size))), frame.counts.y - 1u);
    let info = frame.levels[level];
    let scale = f32(1u << level);
    let a = vec2<i32>(max(floor(first / scale), vec2(0.0)));
    let b = min(vec2<i32>(floor(last / scale)), vec2<i32>(i32(info.y) - 1, i32(info.z) - 1));
    var farthest = 1e30;
    for (var y = a.y; y <= b.y; y++) {
        for (var x = a.x; x <= b.x; x++) {
            farthest = min(farthest, pyramid[info.x + u32(y) * info.y + u32(x)]);
        }
    }
    return farthest > 0.0 && nearest < farthest;
}

@compute @workgroup_size(64)
fn cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let t = id.x;
    if t >= frame.counts.x {
        return;
    }
    let test = tests[t];
    if !hidden(origins[test.x].xyz, test.w) {
        return;
    }
    for (var k = 0u; k < test.z; k++) {
        draws[(test.y + k) * 5u + 1u] = 0u;
    }
    let slot = atomicAdd(&args[1], 1u);
    rescue[slot] = vec4(test.x, test.y + frame.counts.z, test.z, test.w);
}
